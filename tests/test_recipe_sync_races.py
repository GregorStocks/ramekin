"""Recipe sync must not skip a change that commits across its snapshot.

The window a wall-clock cursor loses: a writer takes its change stamp, a sync
runs and hands back a cursor, and only *then* does the writer commit. The
change's timestamp precedes the cursor, so the next `> last_sync_at` delta
excludes it forever. The xid watermark closes it — an in-flight writer's
transaction id is always at or above the cursor, so the next `>= cursor` delta
returns it.

Each test holds that uncommitted change open in its own database transaction
and syncs through the API across it. The writes below mirror the server's write
statements; that the *handlers* stamp `change_xid` correctly is covered
separately by the sequential deltas in `test_recipe_sync.py`.

The change cannot be driven through the API here: a stalled write would block a
Diesel call inside an async handler, and when that handler is running on the
tokio worker holding the IO driver the whole server stops accepting connections
(see `p2-blocking-db-calls-stall-the-server`). The sync under test would never
be answered.
"""

import os

import psycopg
import pytest
import requests

from conftest import make_ingredient
from ramekin_client.api import RecipesApi, TagsApi
from ramekin_client.models import CreateRecipeRequest

SYNC_TIMEOUT_SECONDS = 30


def _sync(client, server_url, cursor=None):
    params = {"limit": 500} if cursor is None else {"limit": 500, "cursor": cursor}
    response = requests.get(
        f"{server_url}/api/recipes/sync",
        headers={"Authorization": f"Bearer {client.configuration.access_token}"},
        params=params,
        timeout=SYNC_TIMEOUT_SECONDS,
    )
    response.raise_for_status()
    return response.json()


@pytest.fixture
def uncommitted():
    """A transaction left open, so its writes are stamped but not yet visible."""
    database_url = os.environ.get("DATABASE_URL")
    if not database_url:
        raise ValueError("DATABASE_URL environment variable required")
    with psycopg.connect(database_url) as conn:  # autocommit off
        yield conn
        conn.rollback()


def test_sync_returns_update_that_commits_across_the_snapshot(
    authed_api_client, server_url, uncommitted
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

    # A new version, stamped but uncommitted — the shape of every recipe edit.
    new_version_id = uncommitted.execute(
        "INSERT INTO recipe_versions (recipe_id, title, description, ingredients,"
        " instructions, source_url, source_name, photo_ids, servings, prep_time,"
        " cook_time, total_time, rating, difficulty, nutritional_info, notes,"
        " version_source)"
        " SELECT recipe_id, %s, description, ingredients, %s, source_url, source_name,"
        " photo_ids, servings, prep_time, cook_time, total_time, rating, difficulty,"
        " nutritional_info, notes, 'manual'"
        " FROM recipe_versions"
        " WHERE id = (SELECT current_version_id FROM recipes WHERE id = %s)"
        " RETURNING id",
        ("After Racing Update", "Simmer it.", created.id),
    ).fetchone()[0]
    uncommitted.execute(
        "UPDATE recipes SET current_version_id = %s WHERE id = %s",
        (new_version_id, created.id),
    )

    racing = _sync(client, server_url)
    assert "After Racing Update" not in {r["title"] for r in racing["recipes"]}

    uncommitted.commit()

    after = _sync(client, server_url, cursor=racing["cursor"])

    recipes_by_id = {recipe["id"]: recipe for recipe in after["recipes"]}
    assert str(created.id) in recipes_by_id
    assert recipes_by_id[str(created.id)]["title"] == "After Racing Update"
    assert recipes_by_id[str(created.id)]["instructions"] == "Simmer it."


def test_sync_returns_soft_delete_that_commits_across_the_snapshot(
    authed_api_client, server_url, uncommitted
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

    uncommitted.execute(
        "UPDATE recipes SET deleted_at = now(), deleted_xid = current_change_xid()"
        " WHERE id = %s",
        (created.id,),
    )

    racing = _sync(client, server_url)
    assert str(created.id) not in racing["deleted"]

    uncommitted.commit()

    after = _sync(client, server_url, cursor=racing["cursor"])

    assert str(created.id) in after["deleted"]
    assert str(created.id) not in {recipe["id"] for recipe in after["recipes"]}


def test_sync_returns_tag_rename_that_commits_across_the_snapshot(
    authed_api_client, server_url, uncommitted
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
    tag = next(
        tag
        for tag in tags_api.list_all_tags().tags
        if tag.name == "before-racing-rename"
    )

    uncommitted.execute(
        "UPDATE user_tags SET name = %s, updated_at = now(),"
        " change_xid = current_change_xid() WHERE id = %s",
        ("after-racing-rename", tag.id),
    )

    racing = _sync(client, server_url)

    uncommitted.commit()

    # The rename never touches the recipe's own version row, so the recipe can
    # only come back through the tag arm of the delta.
    after = _sync(client, server_url, cursor=racing["cursor"])

    recipes_by_id = {recipe["id"]: recipe for recipe in after["recipes"]}
    assert str(created.id) in recipes_by_id
    assert recipes_by_id[str(created.id)]["tags"] == ["after-racing-rename"]
