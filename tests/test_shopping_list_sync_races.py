"""Shopping-list sync must not skip a change that commits across its snapshot.

Same race as `test_recipe_sync_races.py`: a writer stamps `updated_at`, a sync
runs and hands back its cursor, and only *then* does the writer commit. A
wall-clock `last_sync_at` excludes that change forever; the xid cursor
redelivers it because the in-flight writer's transaction id is at or above the
watermark. The racing write queues behind a row lock held by `uncommitted`,
so the choreography is deterministic without sleeps.
"""

from conftest import WRITE_TIMEOUT_SECONDS, blocked_api_write
from ramekin_client.api import ShoppingListApi
from ramekin_client.models import (
    CreateShoppingListItemRequest,
    CreateShoppingListRequest,
    SyncRequest,
    UpdateShoppingListItemRequest,
)


def _create_item(api, name):
    return api.create_items(
        CreateShoppingListRequest(items=[CreateShoppingListItemRequest(item=name)])
    ).ids[0]


def _lock_item(uncommitted, item_id):
    uncommitted.execute(
        "SELECT id FROM shopping_list_items WHERE id = %s FOR UPDATE", (item_id,)
    )


def test_sync_returns_update_that_commits_across_the_snapshot(
    authed_api_client, database_url, uncommitted
):
    client, _user_id = authed_api_client
    api = ShoppingListApi(client)
    item_id = _create_item(api, "before racing update")
    _lock_item(uncommitted, item_id)

    def racing_update():
        api.update_item(
            item_id,
            UpdateShoppingListItemRequest(item="after racing update"),
            _request_timeout=WRITE_TIMEOUT_SECONDS,
        )

    with blocked_api_write(database_url, uncommitted, racing_update):
        racing = api.sync_items(SyncRequest())
    assert "after racing update" not in {c.item for c in racing.server_changes}

    after = api.sync_items(SyncRequest(cursor=racing.cursor))

    changes = {c.id: c for c in after.server_changes}
    assert changes[item_id].item == "after racing update"


def test_sync_returns_soft_delete_that_commits_across_the_snapshot(
    authed_api_client, database_url, uncommitted
):
    client, _user_id = authed_api_client
    api = ShoppingListApi(client)
    item_id = _create_item(api, "racing delete")
    _lock_item(uncommitted, item_id)

    def racing_delete():
        api.delete_item(item_id, _request_timeout=WRITE_TIMEOUT_SECONDS)

    with blocked_api_write(database_url, uncommitted, racing_delete):
        racing = api.sync_items(SyncRequest())
    assert item_id in {c.id for c in racing.server_changes}

    after = api.sync_items(SyncRequest(cursor=racing.cursor))

    assert item_id in after.deleted
    assert item_id not in {c.id for c in after.server_changes}
