"""Recipe sync must not skip a change that commits across its snapshot.

The window a wall-clock cursor loses: a writer takes its change stamp, a sync
runs and hands back a cursor, and only *then* does the writer commit. The
change's timestamp precedes the cursor, so the next `> last_sync_at` delta
excludes it forever. The xid watermark closes it — an in-flight writer's
transaction id is always at or above the cursor, so the next `>= cursor` delta
returns it.

Each test drives the racing write through the real API: a helper transaction
holds a row lock on the write's target, `blocked_api_write` (conftest) starts
the API write in a background thread and waits until it queues on that lock,
the sync runs across the in-flight write, and only then does releasing the
lock let the write commit. That exercises the handlers' own `change_xid`
stamping inside the race window; the sequential deltas in
`test_recipe_sync.py` cover the same stamping outside it. The choreography
stays deterministic without sleeps: the write cannot commit while the helper
transaction holds the lock.
"""

import requests

from conftest import WRITE_TIMEOUT_SECONDS, blocked_api_write, make_ingredient
from ramekin_client.api import RecipesApi, TagsApi
from ramekin_client.models import (
    CreateRecipeRequest,
    RenameTagRequest,
    UpdateRecipeRequest,
)

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


def test_sync_returns_update_that_commits_across_the_snapshot(
    authed_api_client, server_url, database_url, uncommitted
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
    version_id = recipes_api.get_recipe(created.id).version_id

    # The update's INSERT INTO recipe_versions queues here: the new version
    # row's foreign key takes a KEY SHARE lock on the referenced recipes row.
    uncommitted.execute(
        "SELECT id FROM recipes WHERE id = %s FOR UPDATE", (created.id,)
    )

    def racing_update():
        recipes_api.update_recipe(
            created.id,
            UpdateRecipeRequest(
                expected_version_id=version_id,
                title="After Racing Update",
                instructions="Simmer it.",
            ),
            _request_timeout=WRITE_TIMEOUT_SECONDS,
        )

    with blocked_api_write(database_url, uncommitted, racing_update):
        racing = _sync(client, server_url)
    assert "After Racing Update" not in {r["title"] for r in racing["recipes"]}

    after = _sync(client, server_url, cursor=racing["cursor"])

    recipes_by_id = {recipe["id"]: recipe for recipe in after["recipes"]}
    assert str(created.id) in recipes_by_id
    assert recipes_by_id[str(created.id)]["title"] == "After Racing Update"
    assert recipes_by_id[str(created.id)]["instructions"] == "Simmer it."


def test_sync_returns_soft_delete_that_commits_across_the_snapshot(
    authed_api_client, server_url, database_url, uncommitted
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

    # The delete's UPDATE of the recipes row queues on the row lock.
    uncommitted.execute(
        "SELECT id FROM recipes WHERE id = %s FOR UPDATE", (created.id,)
    )

    def racing_delete():
        recipes_api.delete_recipe(created.id, _request_timeout=WRITE_TIMEOUT_SECONDS)

    with blocked_api_write(database_url, uncommitted, racing_delete):
        racing = _sync(client, server_url)
    assert str(created.id) not in racing["deleted"]

    after = _sync(client, server_url, cursor=racing["cursor"])

    assert str(created.id) in after["deleted"]
    assert str(created.id) not in {recipe["id"] for recipe in after["recipes"]}


def test_sync_returns_tag_rename_that_commits_across_the_snapshot(
    authed_api_client, server_url, database_url, uncommitted
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

    # The rename's UPDATE of the user_tags row queues on the row lock.
    uncommitted.execute("SELECT id FROM user_tags WHERE id = %s FOR UPDATE", (tag.id,))

    def racing_rename():
        tags_api.rename_tag(
            tag.id,
            RenameTagRequest(name="after-racing-rename"),
            _request_timeout=WRITE_TIMEOUT_SECONDS,
        )

    with blocked_api_write(database_url, uncommitted, racing_rename):
        racing = _sync(client, server_url)

    # The rename never touches the recipe's own version row, so the recipe can
    # only come back through the tag arm of the delta.
    after = _sync(client, server_url, cursor=racing["cursor"])

    recipes_by_id = {recipe["id"]: recipe for recipe in after["recipes"]}
    assert str(created.id) in recipes_by_id
    assert recipes_by_id[str(created.id)]["tags"] == ["after-racing-rename"]
