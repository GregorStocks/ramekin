import uuid

import pytest

from conftest import make_ingredient
from ramekin_client.api import RecipesApi, SyncApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import (
    CreateRecipeRequest,
    OpKind,
    OpResult,
    ShoppingListFields,
    ShoppingListOp,
)
from shopping_list_ops import add, create, delete, items, sync, update


def test_empty_list(authed_api_client):
    client, _ = authed_api_client
    response = sync(SyncApi(client))
    assert response.changes == []
    assert response.deleted == []
    assert response.full_resync is True


def test_sync_requires_auth(unauthed_api_client):
    with pytest.raises(ApiException) as exc_info:
        sync(SyncApi(unauthed_api_client))
    assert exc_info.value.status == 401


def test_create_items_under_the_clients_ids(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    butter = create("butter", amount="1 cup")
    flour = create("flour", amount="2 cups")

    response = sync(api, butter, flour)

    assert response.results == [OpResult.APPLIED, OpResult.APPLIED]
    # The request's own writes come back in the same response.
    changes = {change.id: change for change in response.changes}
    assert set(changes) == {butter.id, flour.id}
    assert changes[butter.id].item == "butter"
    assert changes[butter.id].amount == "1 cup"
    assert changes[butter.id].is_checked is False


def test_replaying_a_create_is_a_no_op(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    milk = create("milk")
    sync(api, milk)
    sync(api, update(milk.id, is_checked=True))

    # The response to the first create was lost, so the client resends it.
    response = sync(api, milk)

    assert response.results == [OpResult.APPLIED]
    listed = items(api)
    assert len(listed) == 1
    assert listed[milk.id].is_checked is True


def test_creates_sort_last_unless_told_otherwise(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    first, second = add(api, "a", "b")
    placed = create("c", sort_order=100)
    sync(api, placed)
    last = add(api, "d")[0]

    orders = {item_id: item.sort_order for item_id, item in items(api).items()}
    assert orders[first] < orders[second] < orders[placed.id] == 100 < orders[last]


def test_create_with_source_recipe(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    recipe = RecipesApi(client).create_recipe(
        CreateRecipeRequest(
            title="Chocolate Cake",
            instructions="Bake it",
            ingredients=[make_ingredient(item="sugar")],
        )
    )
    sugar = create(
        "sugar",
        amount="1/2 cup",
        source_recipe_id=str(recipe.id),
        source_recipe_title="Chocolate Cake",
    )

    sync(api, sugar)

    item = items(api)[sugar.id]
    assert item.source_recipe_id == recipe.id
    assert item.source_recipe_title == "Chocolate Cake"


def test_a_create_needs_a_nonblank_item(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    for op in [
        ShoppingListOp(op=OpKind.CREATE, id=str(uuid.uuid4())),
        create("  "),
    ]:
        with pytest.raises(ApiException) as exc_info:
            sync(api, op)
        assert exc_info.value.status == 400
    assert items(api) == {}


def test_update_sets_only_the_fields_it_names(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    milk = create("milk", amount="1 gallon", note="2%")
    sync(api, milk)

    response = sync(api, update(milk.id, amount="2 gallons"))

    assert response.results == [OpResult.APPLIED]
    item = items(api)[milk.id]
    assert item.amount == "2 gallons"
    assert item.note == "2%"
    assert item.item == "milk"


def test_updates_from_two_devices_to_different_fields_both_win(authed_api_client):
    """Per-field last-writer-wins: the phone's check-off and the web's
    rename both survive, whatever each device last saw."""
    client, _ = authed_api_client
    api = SyncApi(client)
    eggs = add(api, "eggs")[0]

    sync(api, update(eggs, item="6 eggs"))
    sync(api, update(eggs, is_checked=True))

    item = items(api)[eggs]
    assert item.item == "6 eggs"
    assert item.is_checked is True


def test_the_later_update_to_a_field_wins(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    eggs = add(api, "eggs")[0]

    response = sync(api, update(eggs, amount="6"), update(eggs, amount="12"))

    assert response.results == [OpResult.APPLIED, OpResult.APPLIED]
    assert items(api)[eggs].amount == "12"


def test_blank_clears_a_nullable_field(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    milk = create("milk", amount="1 gallon", note="2%", category_override="Produce")
    sync(api, milk)

    sync(api, update(milk.id, amount="", note="", category_override=""))

    item = items(api)[milk.id]
    assert item.amount is None
    assert item.note is None
    assert item.category_override is None
    assert item.category == "Dairy & Eggs"


def test_check_and_uncheck(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    eggs = add(api, "eggs")[0]

    sync(api, update(eggs, is_checked=True))
    assert items(api)[eggs].is_checked is True
    sync(api, update(eggs, is_checked=False))
    assert items(api)[eggs].is_checked is False


def test_update_of_an_unknown_item_is_gone(authed_api_client):
    client, _ = authed_api_client
    response = sync(SyncApi(client), update(str(uuid.uuid4()), is_checked=True))
    assert response.results == [OpResult.GONE]


def test_delete_wins_over_a_later_update(authed_api_client):
    """An edit from a device that hasn't seen the delete yet changes nothing."""
    client, _ = authed_api_client
    api = SyncApi(client)
    bread = add(api, "bread")[0]
    sync(api, delete(bread))

    response = sync(api, update(bread, is_checked=True))

    assert response.results == [OpResult.GONE]
    assert items(api) == {}


def test_delete(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    bread, kept = add(api, "bread", "jam")
    cursor = sync(api).cursor

    response = sync(api, delete(bread), cursor=cursor)

    assert response.results == [OpResult.APPLIED]
    assert bread in response.deleted
    assert set(items(api)) == {kept}


def test_deleting_twice_is_applied_and_unknown_is_gone(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    bread = add(api, "bread")[0]

    response = sync(api, delete(bread), delete(bread), delete(str(uuid.uuid4())))

    assert response.results == [OpResult.APPLIED, OpResult.APPLIED, OpResult.GONE]


def test_cross_user_isolation(authed_api_client, second_authed_api_client):
    client1, _ = authed_api_client
    client2, _ = second_authed_api_client
    api1 = SyncApi(client1)
    api2 = SyncApi(client2)
    private = create("private_item")
    sync(api1, private)

    assert items(api2) == {}
    response = sync(
        api2,
        update(private.id, is_checked=True),
        delete(private.id),
        # Reusing another user's id can never create the client's row.
        ShoppingListOp(
            op=OpKind.CREATE, id=private.id, fields=ShoppingListFields(item="mine")
        ),
    )
    assert response.results == [OpResult.GONE, OpResult.GONE, OpResult.GONE]
    item = items(api1)[private.id]
    assert item.item == "private_item"
    assert item.is_checked is False


def test_sync_from_cursor_returns_later_changes(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    changed, untouched = add(api, "checked later", "untouched")
    initial = sync(api)

    sync(api, update(changed, is_checked=True))
    added = add(api, "added")[0]

    response = sync(api, cursor=initial.cursor)
    changes = {change.id: change for change in response.changes}
    assert changes[changed].is_checked
    assert added in changes
    assert untouched not in changes
    assert response.cursor > initial.cursor
    assert initial.full_resync is True
    assert response.full_resync is False


def test_own_writes_are_not_redelivered(authed_api_client):
    """Writes come back in their own response, and the cursor it returns is
    past them."""
    client, _ = authed_api_client
    api = SyncApi(client)
    cursor = sync(api).cursor
    milk = create("milk")

    written = sync(api, milk, cursor=cursor)
    assert [change.id for change in written.changes] == [milk.id]

    after = sync(api, cursor=written.cursor)
    assert after.changes == []
    assert after.deleted == []


def test_change_seq_grows_with_each_change(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    eggs = add(api, "eggs")[0]
    first = items(api)[eggs].change_seq

    sync(api, update(eggs, is_checked=True))

    assert items(api)[eggs].change_seq > first


def test_full_resync_sends_no_tombstones(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    kept, removed = add(api, "kept", "removed")
    sync(api, delete(removed))

    response = sync(api)

    assert set(change.id for change in response.changes) == {kept}
    assert response.deleted == []
    assert response.full_resync is True


def test_sync_answers_a_cursor_from_another_database_with_a_full_resync(
    authed_api_client,
):
    """A cursor above any this database has issued (a client that synced
    before a restore from an older backup) gets every live item, flagged so
    the client drops cached items this database doesn't have."""
    client, _ = authed_api_client
    api = SyncApi(client)
    kept, removed = add(api, "kept", "removed")
    sync(api, delete(removed))
    current = sync(api).cursor

    response = sync(api, cursor=current + 1_000_000_000)

    assert {change.id for change in response.changes} == {kept}
    assert response.deleted == []
    assert response.full_resync is True


def test_items_have_category(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    add(
        api,
        "chicken breast",
        "olive oil",
        "fresh basil",
        "parmesan cheese",
        "unknown_xyz_ingredient",
    )

    by_name = {item.item: item for item in items(api).values()}

    assert by_name["chicken breast"].category == "Meat & Seafood"
    assert by_name["olive oil"].category == "Oils & Vinegars"
    assert by_name["fresh basil"].category == "Produce"
    assert by_name["parmesan cheese"].category == "Cheese"
    assert by_name["unknown_xyz_ingredient"].category == "Other"


def test_renaming_an_item_stores_the_new_names_category(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    oil = add(api, "olive oil")[0]

    response = sync(api, update(oil, item="chicken breast"))

    change = {c.id: c for c in response.changes}[oil]
    assert change.computed_category == "Meat & Seafood"
    assert items(api)[oil].category == "Meat & Seafood"


def test_category_override_wins_over_computed_category(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    chicken = create("chicken breast", category_override="Produce")
    sync(api, chicken)

    item = items(api)[chicken.id]
    assert item.category == "Produce"
    assert item.category_override == "Produce"
    assert item.computed_category == "Meat & Seafood"

    sync(api, update(chicken.id, category_override="Cheese"))
    assert items(api)[chicken.id].category == "Cheese"


def test_rejects_unknown_category_override(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    butter = add(api, "butter")[0]

    for op in [
        update(butter, category_override="Not A Category"),
        create("milk", category_override="Not A Category"),
    ]:
        with pytest.raises(ApiException) as exc_info:
            # The whole request fails, valid ops included.
            sync(api, update(butter, is_checked=True), op)
        assert exc_info.value.status == 400
    assert items(api)[butter].is_checked is False


EXPECTED_CATEGORY_ORDER = [
    "Produce",
    "Meat & Seafood",
    "Dairy & Eggs",
    "Cheese",
    "Bakery & Bread",
    "Frozen",
    "Pasta & Rice",
    "Canned Goods",
    "Baking",
    "Spices & Seasonings",
    "Condiments & Sauces",
    "Oils & Vinegars",
    "Nuts & Dried Fruit",
    "Beverages",
    "Snacks",
    "Household",
    "Personal Care",
    "Pet",
    "Other",
]


def test_sync_includes_category_order(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    add(api, "chicken breast", "unknown_xyz_ingredient")

    response = sync(api)

    assert response.category_order == EXPECTED_CATEGORY_ORDER
    for item in response.changes:
        assert item.category in response.category_order
