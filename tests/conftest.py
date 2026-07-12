import concurrent.futures
import contextlib
import os
import re
import threading
import time
import uuid

import psycopg
import pytest

from query_thresholds import get_thresholds
from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, PhotosApi, TestingApi
from ramekin_client.models import Ingredient, Measurement, SignupRequest


def _require_fixture_base_url() -> str:
    """Return the FIXTURE_BASE_URL env var, raising if unset."""
    base = os.environ.get("FIXTURE_BASE_URL")
    if not base:
        raise ValueError("FIXTURE_BASE_URL environment variable is not set")
    return base


@pytest.fixture(scope="session")
def fixture_base_url() -> str:
    return _require_fixture_base_url()


def wait_for_job_completion(scrape_api, job_id: str, timeout: float = 30.0):
    """Poll ``get_scrape`` until the job reaches a terminal status.

    Uses monotonic time so clock adjustments don't affect the timeout.
    """
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        job = scrape_api.get_scrape(job_id)
        if job.status in ("completed", "failed"):
            return job
        time.sleep(0.25)
    raise TimeoutError(f"scrape {job_id} did not finish in {timeout}s")


def make_ingredient(
    item: str,
    amount: str | None = None,
    unit: str | None = None,
    note: str | None = None,
) -> Ingredient:
    """Create an Ingredient with the new measurements structure."""
    measurements = []
    if amount is not None or unit is not None:
        measurements.append(Measurement(amount=amount, unit=unit))
    return Ingredient(item=item, measurements=measurements, note=note)


@pytest.fixture
def server_url():
    api_base_url = os.environ.get("API_BASE_URL")
    if not api_base_url:
        raise ValueError("API_BASE_URL environment variable required")
    return api_base_url


@pytest.fixture
def database_url():
    url = os.environ.get("DATABASE_URL")
    if not url:
        raise ValueError("DATABASE_URL environment variable required")
    return url


@pytest.fixture
def uncommitted(database_url):
    """A transaction left open, so its writes and row locks stay pending.

    Rolled back at teardown; commit explicitly in the test to release it.
    """
    with psycopg.connect(database_url) as conn:  # autocommit off
        yield conn
        conn.rollback()


LOCK_WAIT_TIMEOUT_SECONDS = 30
WRITE_TIMEOUT_SECONDS = 60


def _wait_until_blocked_behind(database_url, holder_pid, write):
    """Wait for some backend to queue on a lock held by backend `holder_pid`.

    Matches on the holder's backend pid via `pg_blocking_pids` rather than on
    query text, so concurrently running tests (pytest-xdist) can't satisfy the
    wait by accident. Returns early if the write future finishes, so a write
    that fails outright surfaces its error instead of burning the timeout.
    """
    deadline = time.monotonic() + LOCK_WAIT_TIMEOUT_SECONDS
    with psycopg.connect(database_url, autocommit=True) as conn:
        while time.monotonic() < deadline:
            if write.done():
                return
            waiting = conn.execute(
                "SELECT count(*) FROM pg_stat_activity"
                " WHERE wait_event_type = 'Lock'"
                " AND %s = ANY(pg_blocking_pids(pid))",
                (holder_pid,),
            ).fetchone()[0]
            if waiting:
                return
            time.sleep(0.1)
    raise TimeoutError("the write never started waiting on the row lock")


def _run_in_daemon_thread(fn):
    """Run `fn` on a daemon thread, exposing its outcome as a Future.

    A daemon thread rather than a ThreadPoolExecutor worker: nothing joins the
    thread, so a write wedged past its timeout fails the test instead of
    hanging the process in executor shutdown or interpreter exit.
    """
    future = concurrent.futures.Future()

    def run():
        try:
            future.set_result(fn())
        except BaseException as exc:
            future.set_exception(exc)

    threading.Thread(target=run, daemon=True).start()
    return future


@contextlib.contextmanager
def blocked_api_write(database_url, uncommitted, send_write):
    """Hold an API write in flight, queued behind a row lock.

    `uncommitted` must already hold a lock (e.g. `SELECT ... FOR UPDATE`) on a
    row that `send_write` will touch. The write runs in a background thread;
    the body of the `with` block runs once the write is queued on the lock and
    therefore cannot commit until the block exits. On exit the lock is
    released so the write can finish, and any exception it raised propagates.
    Yields the write's Future; its result is available after the block.
    """
    holder_pid = uncommitted.execute("SELECT pg_backend_pid()").fetchone()[0]
    write = _run_in_daemon_thread(send_write)
    try:
        _wait_until_blocked_behind(database_url, holder_pid, write)
        if write.done():
            write.result()  # surfaces the failed write's own error
            # It cannot have committed while the lock was held, so finishing
            # means the lock never covered the write at all.
            raise AssertionError("the write finished without queuing on the row lock")
        yield write
    finally:
        # Release the lock even on failure so the write can finish.
        uncommitted.rollback()
    write.result(timeout=WRITE_TIMEOUT_SECONDS)


@pytest.fixture
def api_config(server_url):
    return Configuration(host=server_url)


@pytest.fixture
def unauthed_api_client(api_config):
    with ApiClient(api_config) as client:
        yield client


@pytest.fixture
def auth_api(unauthed_api_client):
    return AuthApi(unauthed_api_client)


@pytest.fixture
def testing_api(unauthed_api_client):
    return TestingApi(unauthed_api_client)


@pytest.fixture
def photos_api(unauthed_api_client):
    return PhotosApi(unauthed_api_client)


@pytest.fixture
def unique_username():
    return f"testuser_{uuid.uuid4().hex[:8]}"


@pytest.fixture
def authed_api_client(api_config, auth_api, unique_username):
    response = auth_api.signup(
        SignupRequest(username=unique_username, password="testpass123")
    )
    api_config.access_token = response.token
    with ApiClient(api_config) as client:
        yield client, response.user_id


@pytest.fixture
def second_authed_api_client(api_config, auth_api):
    """A second authenticated user for testing cross-user isolation."""
    username = f"testuser2_{uuid.uuid4().hex[:8]}"
    response = auth_api.signup(SignupRequest(username=username, password="testpass123"))
    config = Configuration(host=api_config.host)
    config.access_token = response.token
    with ApiClient(config) as client:
        yield client, response.user_id


@pytest.fixture
def test_image():
    """Load a test image from the seed images directory."""
    image_path = os.path.join(
        os.path.dirname(__file__), "..", "cli", "src", "seed_images", "bread.png"
    )
    with open(image_path, "rb") as f:
        return f.read()


# --- Query Count Tracking ---

# UUID pattern for normalizing URLs to endpoint patterns
_UUID_PATTERN = re.compile(
    r"/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"
)


def _normalize_endpoint(method: str, url: str) -> str:
    """Convert a concrete URL to its endpoint pattern.

    Example: "GET http://localhost:55372/api/recipes/abc-123-..."
             -> "GET /api/recipes/{id}"
    """
    # Remove host prefix
    path = re.sub(r"^https?://[^/]+", "", url)
    # Replace UUIDs with {id}
    path = _UUID_PATTERN.sub("/{id}", path)
    return f"{method} {path}"


class QueryCountTracker:
    """Tracks database query counts per API call and asserts thresholds.

    Records query counts from X-DB-Query-Count response headers and fails
    if any endpoint falls outside its (min, max) threshold range.

    - Exceeding max indicates a potential N+1 query regression
    - Going below min indicates an efficiency improvement (update thresholds!)
    """

    def __init__(self):
        self.violations: list[
            tuple[str, int, int, int]
        ] = []  # (endpoint, actual, min, max)

    def record(self, method: str, url: str, headers: dict) -> int | None:
        """Record query count from response headers. Returns the count if found."""
        count_str = headers.get("X-DB-Query-Count") or headers.get("x-db-query-count")
        if not count_str:
            return None

        count = int(count_str)
        endpoint = _normalize_endpoint(method, url)
        min_threshold, max_threshold = get_thresholds(endpoint)

        if count < min_threshold or count > max_threshold:
            self.violations.append((endpoint, count, min_threshold, max_threshold))

        return count

    def assert_ok(self):
        """Raise AssertionError if any endpoints violated thresholds."""
        if not self.violations:
            return

        lines = []
        for endpoint, actual, min_t, max_t in self.violations:
            if actual < min_t:
                lines.append(
                    f"  {endpoint}: {actual} queries < min {min_t} "
                    "(endpoint got more efficient - update thresholds!)"
                )
            else:
                lines.append(
                    f"  {endpoint}: {actual} queries > max {max_t} "
                    "(potential N+1 query regression)"
                )

        raise AssertionError("Query count threshold violations:\n" + "\n".join(lines))


@pytest.fixture
def query_tracker():
    """Fixture that tracks query counts and asserts thresholds on teardown.

    Usage:
        def test_something(authed_api_client, query_tracker):
            client, user_id = authed_api_client
            api = RecipesApi(client)

            response = api.list_recipes_with_http_info()
            query_tracker.record(
                "GET",
                f"{client.configuration.host}/api/recipes",
                dict(response.headers),
            )

            assert response.data.recipes == []
    """
    tracker = QueryCountTracker()
    yield tracker
    tracker.assert_ok()
