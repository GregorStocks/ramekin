"""Shopping-list sync must not skip a change that commits across its snapshot.

Same race as `test_recipe_sync_races.py`: a writer takes its change stamp, a
sync runs and hands back its cursor, and only *then* does the writer commit. A
wall-clock `last_sync_at` excludes that change forever; the xid cursor
redelivers it because the in-flight writer's transaction id is at or above the
watermark.

The racing writer is itself a sync carrying an offline create plus a change to
an item that `uncommitted` holds a row lock on. The create's INSERT assigns the
writer its xid before the second statement queues on the lock, so the xid
predates the racing snapshot — the case the watermark exists for. (A lone
UPDATE would not do: the `change_xid` BEFORE UPDATE trigger makes Postgres lock
the row before assigning an xid, and an xid assigned after the racing snapshot
is trivially above its watermark.)
"""

import uuid

from conftest import WRITE_TIMEOUT_SECONDS, blocked_api_write
from ramekin_client.api import ShoppingListApi
from ramekin_client.models import (
    CreateShoppingListItemRequest,
    CreateShoppingListRequest,
    SyncCreateItem,
    SyncRequest,
    SyncUpdateItem,
)


def _create_item(api, name):
    return api.create_items(
        CreateShoppingListRequest(items=[CreateShoppingListItemRequest(item=name)])
    ).ids[0]


def _lock_item(uncommitted, item_id):
    uncommitted.execute(
        "SELECT id FROM shopping_list_items WHERE id = %s FOR UPDATE", (item_id,)
    )


def _offline_create(name):
    return SyncCreateItem(
        client_id=str(uuid.uuid4()), item=name, is_checked=False, sort_order=0
    )


def test_sync_returns_update_that_commits_across_the_snapshot(
    authed_api_client, database_url, uncommitted
):
    client, _user_id = authed_api_client
    api = ShoppingListApi(client)
    item_id = _create_item(api, "before racing update")
    _lock_item(uncommitted, item_id)

    def racing_update():
        api.sync_items(
            SyncRequest(
                creates=[_offline_create("racing create")],
                updates=[
                    SyncUpdateItem(
                        id=item_id, item="after racing update", expected_version=1
                    )
                ],
            ),
            _request_timeout=WRITE_TIMEOUT_SECONDS,
        )

    with blocked_api_write(database_url, uncommitted, racing_update):
        racing = api.sync_items(SyncRequest())
    racing_items = {c.item for c in racing.server_changes}
    assert "after racing update" not in racing_items
    assert "racing create" not in racing_items

    after = api.sync_items(SyncRequest(cursor=racing.cursor))

    after_items = {c.item for c in after.server_changes}
    assert {"after racing update", "racing create"} <= after_items


def test_sync_returns_soft_delete_that_commits_across_the_snapshot(
    authed_api_client, database_url, uncommitted
):
    client, _user_id = authed_api_client
    api = ShoppingListApi(client)
    item_id = _create_item(api, "racing delete")
    _lock_item(uncommitted, item_id)

    def racing_delete():
        api.sync_items(
            SyncRequest(
                creates=[_offline_create("created before delete")], deletes=[item_id]
            ),
            _request_timeout=WRITE_TIMEOUT_SECONDS,
        )

    with blocked_api_write(database_url, uncommitted, racing_delete):
        racing = api.sync_items(SyncRequest())
    assert item_id in {c.id for c in racing.server_changes}

    after = api.sync_items(SyncRequest(cursor=racing.cursor))

    assert item_id in after.deleted
    assert item_id not in {c.id for c in after.server_changes}
