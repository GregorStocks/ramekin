"""The shopping list's history and undo, and the `restore` sync op."""

import uuid

import pytest

from ramekin_client.api import ShoppingListApi, SyncApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import MealPlanOp, OpKind, OpResult, SyncMealPlansRequest
from shopping_list_ops import add, create, delete, items, restore, sync, update


def history(client):
    return ShoppingListApi(client).list_shopping_list_history().entries


def undo(client, entry_id):
    return ShoppingListApi(client).undo_shopping_list_change(entry_id)


def test_restore_brings_a_deleted_item_back_as_it_was(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    milk = create("milk", amount="1 gallon")
    sync(api, milk, update(milk.id, is_checked=True))
    sync(api, delete(milk.id))
    cursor = sync(api).cursor

    response = sync(api, restore(milk.id), cursor=cursor)

    assert response.results == [OpResult.APPLIED]
    assert [change.id for change in response.changes] == [milk.id]
    restored = items(api)[milk.id]
    assert restored.amount == "1 gallon"
    assert restored.is_checked is True


def test_restore_sets_the_fields_it_names(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    eggs = add(api, "eggs")[0]
    sync(api, delete(eggs))

    sync(api, restore(eggs, is_checked=True))

    assert items(api)[eggs].is_checked is True


def test_restoring_a_live_item_just_sets_its_fields(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    eggs = add(api, "eggs")[0]

    response = sync(api, restore(eggs, amount="12"))

    assert response.results == [OpResult.APPLIED]
    assert items(api)[eggs].amount == "12"


def test_restoring_an_unknown_or_foreign_item_is_gone(
    authed_api_client, second_authed_api_client
):
    client1, _ = authed_api_client
    client2, _ = second_authed_api_client
    private = add(SyncApi(client1), "private")[0]
    sync(SyncApi(client1), delete(private))

    response = sync(
        SyncApi(client2),
        restore(private, item="mine"),
        restore(str(uuid.uuid4())),
    )

    assert response.results == [OpResult.GONE, OpResult.GONE]
    assert items(SyncApi(client1)) == {}
    assert items(SyncApi(client2)) == {}


def test_restoring_an_item_whose_create_never_arrived_creates_it(
    authed_api_client,
):
    """An offline client added, deleted and restored an item before syncing."""
    client, _ = authed_api_client
    api = SyncApi(client)
    unsent = uuid.uuid4()

    response = sync(api, restore(unsent, item="milk", amount="", is_checked=True))

    assert response.results == [OpResult.APPLIED]
    restored = items(api)[unsent]
    assert restored.item == "milk"
    assert restored.amount is None
    assert restored.is_checked is True


def test_meal_plans_cannot_be_restored(authed_api_client):
    client, _ = authed_api_client
    with pytest.raises(ApiException) as exc_info:
        SyncApi(client).sync_meal_plans(
            SyncMealPlansRequest(
                ops=[MealPlanOp(op=OpKind.RESTORE, id=str(uuid.uuid4()))]
            )
        )
    assert exc_info.value.status == 400


def test_each_sync_that_changes_the_list_is_one_entry(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    milk, eggs, bread = add(api, "milk", "eggs", "bread")
    sync(api, update(milk, is_checked=True), update(eggs, is_checked=True))
    sync(api, delete(milk), delete(eggs))
    # Changes nothing, so it isn't history.
    sync(api)
    sync(api, update(bread, item="bread"))

    assert [entry.summary for entry in history(client)] == [
        "Deleted milk and eggs",
        "Checked milk and eggs",
        "Added milk, eggs and bread",
    ]


def test_history_is_private(authed_api_client, second_authed_api_client):
    client1, _ = authed_api_client
    client2, _ = second_authed_api_client
    add(SyncApi(client1), "milk")
    entry = history(client1)[0]

    assert history(client2) == []
    with pytest.raises(ApiException) as exc_info:
        undo(client2, entry.id)
    assert exc_info.value.status == 404


def test_history_limit(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    for name in ["a", "b", "c"]:
        add(api, name)

    entries = ShoppingListApi(client).list_shopping_list_history(limit=2).entries

    assert [entry.summary for entry in entries] == ["Added c", "Added b"]


def test_undoing_a_clear_restores_the_cleared_items(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    milk, eggs, bread = add(api, "milk", "eggs", "bread")
    sync(api, update(milk, is_checked=True), update(eggs, is_checked=True))
    sync(api, delete(milk), delete(eggs))
    cleared = history(client)[0]
    cursor = sync(api).cursor

    entry = undo(client, cleared.id)

    assert entry.undoes_id == cleared.id
    assert entry.summary == "Restored eggs and milk"
    response = sync(api, cursor=cursor)
    assert {change.id for change in response.changes} == {milk, eggs}
    listed = items(api)
    assert set(listed) == {milk, eggs, bread}
    assert listed[milk].is_checked is True
    assert history(client)[1].undone_by_id == entry.id


def test_undoing_an_add_deletes_the_items(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    add(api, "milk", "eggs")

    undo(client, history(client)[0].id)

    assert items(api) == {}


def test_undoing_an_edit_leaves_fields_changed_since(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    milk = add(api, "milk")[0]
    sync(api, update(milk, item="oat milk", amount="1 quart"))
    edit = history(client)[0]
    sync(api, update(milk, item="soy milk"))

    undo(client, edit.id)

    undone = items(api)[milk]
    assert undone.item == "soy milk"
    assert undone.amount is None


def test_an_entry_can_be_undone_once_and_its_undo_undone(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    milk = add(api, "milk")[0]
    sync(api, update(milk, is_checked=True))
    check = history(client)[0]

    unchecked = undo(client, check.id)
    with pytest.raises(ApiException) as exc_info:
        undo(client, check.id)
    assert exc_info.value.status == 409
    assert items(api)[milk].is_checked is False

    # Undoing the undo redoes the check.
    undo(client, unchecked.id)
    assert items(api)[milk].is_checked is True


def test_undoing_an_entry_later_changes_replaced_is_a_conflict(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    milk = add(api, "milk")[0]
    sync(api, update(milk, is_checked=True))
    check = history(client)[0]
    sync(api, update(milk, is_checked=False))

    with pytest.raises(ApiException) as exc_info:
        undo(client, check.id)
    assert exc_info.value.status == 409


def test_undoing_an_unknown_entry_is_not_found(authed_api_client):
    client, _ = authed_api_client
    with pytest.raises(ApiException) as exc_info:
        undo(client, str(uuid.uuid4()))
    assert exc_info.value.status == 404
