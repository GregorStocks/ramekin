"""Recipe sync must not skip a change that commits across its snapshot.

Each test stalls a real API write mid-transaction by holding a row lock the
write must take. The write therefore acquires its change stamp, a sync runs and
returns a cursor, and only then does the write commit. The change lands on the
far side of that sync's snapshot, which is exactly the window a wall-clock
cursor loses: the row's timestamp precedes the cursor, so a later
`> last_sync_at` delta excludes it forever.

These tests are pinned to one xdist group so at most one API request is ever
blocked on a lock: handlers hold their worker thread across the query.
"""

import os
import time
from concurrent.futures import ThreadPoolExecutor
from contextlib import contextmanager

import psycopg
import pytest
import requests

from conftest import make_ingredient
from ramekin_client.api import RecipesApi, TagsApi
from ramekin_client.models import (
    CreateRecipeRequest,
    RenameTagRequest,
    UpdateRecipeRequest,
)

pytestmark = pytest.mark.xdist_group("recipe_sync_races")

BLOCKED_TIMEOUT_SECONDS = 20
WRITE_TIMEOUT_SECONDS = 30


def _sync(client, server_url, cursor=None):
    params = {} if cursor is None else {"cursor": cursor}
    response = requests.get(
        f"{server_url}/api/recipes/sync",
        headers={"Authorization": f"Bearer {client.configuration.access_token}"},
        params=params,
        timeout=WRITE_TIMEOUT_SECONDS,
    )
    response.raise_for_status()
    return response.json()


class WriteBlocker:
    """Holds a row lock so the API write that needs it stalls before commit."""

    def __init__(self, conn):
        self._conn = conn
        self._xid = None

    def lock_recipe(self, recipe_id):
        """Block the recipes UPDATE that an update or soft delete performs."""
        self._lock("SELECT id FROM recipes WHERE id = %s FOR UPDATE", recipe_id)

    def lock_tag(self, tag_id):
        """Block the user_tags UPDATE that a rename performs."""
        self._lock("SELECT id FROM user_tags WHERE id = %s FOR UPDATE", tag_id)

    def _lock(self, sql, row_id):
        assert self._conn.execute(sql, (row_id,)).fetchone() is not None
        # Locking stamped our transaction id onto the row. The blocked writer
        # waits on that id, which identifies our stall precisely enough to
        # ignore any other test's locks.
        self._xid = self._conn.execute("SELECT pg_current_xact_id()::text").fetchone()[
            0
        ]

    def wait_until_write_blocked(self):
        deadline = time.monotonic() + BLOCKED_TIMEOUT_SECONDS
        while time.monotonic() < deadline:
            waiting = self._conn.execute(
                "SELECT count(*) FROM pg_locks "
                "WHERE NOT granted AND locktype = 'transactionid' "
                "AND transactionid = %s::xid",
                (self._xid,),
            ).fetchone()[0]
            if waiting:
                return
            time.sleep(0.05)
        raise TimeoutError("API write never blocked on the held row lock")

    def release(self):
        self._conn.rollback()


@pytest.fixture
def blocker():
    database_url = os.environ.get("DATABASE_URL")
    if not database_url:
        raise ValueError("DATABASE_URL environment variable required")
    with psycopg.connect(database_url) as conn:
        yield WriteBlocker(conn)
        conn.rollback()


@contextmanager
def stalled_api_write(blocker, take_lock, write):
    """Stall an API write mid-transaction, then commit it on exit.

    The body runs while `write` holds its change stamp but has not committed.
    The lock is released in a `finally` so a failing assertion surfaces as a
    failure instead of hanging the executor on a write that can never finish.
    """
    take_lock()
    with ThreadPoolExecutor(max_workers=1) as pool:
        future = pool.submit(write)
        try:
            blocker.wait_until_write_blocked()
            yield
        finally:
            blocker.release()
            future.result(timeout=WRITE_TIMEOUT_SECONDS)


def test_sync_returns_update_that_commits_across_the_snapshot(
    authed_api_client, server_url, blocker
):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)

    created = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Before Racing Update",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="rice")],
        )
    )
    _sync(client, server_url)

    update = UpdateRecipeRequest(
        title="After Racing Update",
        instructions="Simmer it.",
        ingredients=[make_ingredient(item="brown rice")],
    )
    with stalled_api_write(
        blocker,
        lambda: blocker.lock_recipe(created.id),
        lambda: recipes_api.update_recipe(created.id, update),
    ):
        racing = _sync(client, server_url)
        assert "After Racing Update" not in {r["title"] for r in racing["recipes"]}

    after = _sync(client, server_url, cursor=racing["cursor"])

    recipes_by_id = {recipe["id"]: recipe for recipe in after["recipes"]}
    assert str(created.id) in recipes_by_id
    assert recipes_by_id[str(created.id)]["title"] == "After Racing Update"
    assert recipes_by_id[str(created.id)]["instructions"] == "Simmer it."


def test_sync_returns_soft_delete_that_commits_across_the_snapshot(
    authed_api_client, server_url, blocker
):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)

    created = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Racing Delete",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="beans")],
        )
    )
    _sync(client, server_url)

    with stalled_api_write(
        blocker,
        lambda: blocker.lock_recipe(created.id),
        lambda: recipes_api.delete_recipe(created.id),
    ):
        racing = _sync(client, server_url)
        assert str(created.id) not in racing["deleted"]

    after = _sync(client, server_url, cursor=racing["cursor"])

    assert str(created.id) in after["deleted"]
    assert str(created.id) not in {recipe["id"] for recipe in after["recipes"]}


def test_sync_returns_tag_rename_that_commits_across_the_snapshot(
    authed_api_client, server_url, blocker
):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    tags_api = TagsApi(client)

    created = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Racing Tag Rename",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="chickpeas")],
            tags=["before-racing-rename"],
        )
    )
    _sync(client, server_url)
    tag = next(
        tag
        for tag in tags_api.list_all_tags().tags
        if tag.name == "before-racing-rename"
    )

    rename = RenameTagRequest(name="after-racing-rename")
    with stalled_api_write(
        blocker,
        lambda: blocker.lock_tag(tag.id),
        lambda: tags_api.rename_tag(tag.id, rename),
    ):
        racing = _sync(client, server_url)

    # The rename never touches the recipe's own version row, so the recipe can
    # only come back through the tag arm of the delta.
    after = _sync(client, server_url, cursor=racing["cursor"])

    recipes_by_id = {recipe["id"]: recipe for recipe in after["recipes"]}
    assert str(created.id) in recipes_by_id
    assert recipes_by_id[str(created.id)]["tags"] == ["after-racing-rename"]
