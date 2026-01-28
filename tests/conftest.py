import os
import re
import sys
import uuid

import pytest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "generated"))

from query_thresholds import get_thresholds
from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, PhotosApi, TestingApi
from ramekin_client.models import Ingredient, Measurement, SignupRequest


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
