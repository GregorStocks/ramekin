"""The server must keep answering while one query is stuck.

Regression test for a whole-server stall: handlers used to run their Diesel
calls directly on a tokio worker, so a single query waiting on a row lock
parked the worker -- and when that worker held the IO driver, the server
stopped accepting connections entirely. Even endpoints that touch no database
went unanswered. Database work now runs on the blocking thread pool
(`db::run_blocking`), so a stalled write hangs only its own request.

The test wedges a recipe update on a row lock held by an uncommitted
transaction, then requires that a database-free endpoint still answers over
fresh connections while the write is waiting.
"""

import threading
import time

import psycopg
import requests

from conftest import make_ingredient
from ramekin_client.api import RecipesApi
from ramekin_client.models import CreateRecipeRequest

PING_TIMEOUT_SECONDS = 5
LOCK_WAIT_TIMEOUT_SECONDS = 30
UPDATE_TIMEOUT_SECONDS = 60


def _wait_until_update_blocks(database_url):
    """Wait for a backend to be stuck on a lock from the recipe update.

    The update blocks at its `INSERT INTO recipe_versions` statement: the new
    version row's foreign key takes a KEY SHARE lock on the referenced recipes
    row, which conflicts with the test's FOR UPDATE lock. So match any
    recipe-ish statement, not just writes to the recipes table itself.
    """
    deadline = time.monotonic() + LOCK_WAIT_TIMEOUT_SECONDS
    with psycopg.connect(database_url, autocommit=True) as conn:
        while time.monotonic() < deadline:
            waiting = conn.execute(
                "SELECT count(*) FROM pg_stat_activity"
                " WHERE wait_event_type = 'Lock' AND query ILIKE '%recipe%'"
            ).fetchone()[0]
            if waiting:
                return
            time.sleep(0.1)
    raise TimeoutError("the update never started waiting on the row lock")


def test_unrelated_requests_answer_while_a_write_waits_on_a_lock(
    authed_api_client, server_url, database_url, uncommitted
):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)

    created = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Locked Recipe",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="lentils")],
        )
    )

    uncommitted.execute(
        "SELECT id FROM recipes WHERE id = %s FOR UPDATE", (created.id,)
    )

    # The stalled write must arrive over a warm keep-alive connection: that is
    # the shape that used to wedge the IO driver (the handler ran on the very
    # worker that owned it) and take down the whole server.
    session = requests.Session()
    headers = {"Authorization": f"Bearer {client.configuration.access_token}"}
    warm = session.get(
        f"{server_url}/api/recipes/{created.id}", headers=headers, timeout=10
    )
    assert warm.status_code == 200

    update_result = {}

    def blocked_update():
        update_result["response"] = session.put(
            f"{server_url}/api/recipes/{created.id}",
            headers=headers,
            json={
                "expected_version_id": warm.json()["version_id"],
                "title": "Unlocked Recipe",
            },
            timeout=UPDATE_TIMEOUT_SECONDS,
        )

    updater = threading.Thread(target=blocked_update)
    updater.start()
    try:
        _wait_until_update_blocks(database_url)

        # No database, and a fresh TCP connection each time: exactly what went
        # unanswered before database work moved off the runtime threads.
        for _ in range(3):
            ping = requests.get(
                f"{server_url}/api/test/unauthed-ping",
                timeout=PING_TIMEOUT_SECONDS,
            )
            assert ping.status_code == 200
    finally:
        uncommitted.rollback()
        updater.join(timeout=UPDATE_TIMEOUT_SECONDS)

    assert not updater.is_alive()
    assert update_result["response"].status_code == 200
    updated = recipes_api.get_recipe(created.id)
    assert updated.title == "Unlocked Recipe"
