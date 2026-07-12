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

import requests

from conftest import WRITE_TIMEOUT_SECONDS, blocked_api_write, make_ingredient
from ramekin_client.api import RecipesApi
from ramekin_client.models import CreateRecipeRequest

PING_TIMEOUT_SECONDS = 5


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

    # The update's INSERT INTO recipe_versions queues here: the new version
    # row's foreign key takes a KEY SHARE lock on the referenced recipes row.
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

    def blocked_update():
        return session.put(
            f"{server_url}/api/recipes/{created.id}",
            headers=headers,
            json={
                "expected_version_id": warm.json()["version_id"],
                "title": "Unlocked Recipe",
            },
            timeout=WRITE_TIMEOUT_SECONDS,
        )

    with blocked_api_write(database_url, uncommitted, blocked_update) as write:
        # No database, and a fresh TCP connection each time: exactly what went
        # unanswered before database work moved off the runtime threads.
        for _ in range(3):
            ping = requests.get(
                f"{server_url}/api/test/unauthed-ping",
                timeout=PING_TIMEOUT_SECONDS,
            )
            assert ping.status_code == 200

    assert write.result().status_code == 200
    updated = recipes_api.get_recipe(created.id)
    assert updated.title == "Unlocked Recipe"
