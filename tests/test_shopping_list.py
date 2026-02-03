import uuid

import pytest

from conftest import make_ingredient
from ramekin_client.api import RecipesApi, ShoppingListApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import (
    CreateRecipeRequest,
    CreateShoppingListItemRequest,
    CreateShoppingListRequest,
    SyncCreateItem,
    SyncRequest,
    SyncUpdateItem,
    UpdateShoppingListItemRequest,
)


def test_list_shopping_list_empty(authed_api_client):
    """Test listing shopping list when user has none."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    response = api.list_items()
    assert response.items == []


def test_list_shopping_list_requires_auth(unauthed_api_client):
    """Test that listing shopping list requires authentication."""
    api = ShoppingListApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        api.list_items()

    assert exc_info.value.status == 401


def test_create_shopping_list_item(authed_api_client):
    """Test creating shopping list items."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    response = api.create_items(
        CreateShoppingListRequest(
            items=[
                CreateShoppingListItemRequest(item="butter", amount="1 cup"),
                CreateShoppingListItemRequest(item="flour", amount="2 cups"),
            ]
        )
    )

    assert len(response.ids) == 2

    # Verify they appear in list
    list_response = api.list_items()
    assert len(list_response.items) == 2
    items_by_name = {item.item: item for item in list_response.items}
    assert "butter" in items_by_name
    assert items_by_name["butter"].amount == "1 cup"
    assert "flour" in items_by_name


def test_create_shopping_list_requires_auth(unauthed_api_client):
    """Test that creating shopping list requires authentication."""
    api = ShoppingListApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        api.create_items(
            CreateShoppingListRequest(
                items=[CreateShoppingListItemRequest(item="test")]
            )
        )

    assert exc_info.value.status == 401


def test_create_shopping_list_empty_fails(authed_api_client):
    """Test that creating with empty items list fails."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    with pytest.raises(ApiException) as exc_info:
        api.create_items(CreateShoppingListRequest(items=[]))

    assert exc_info.value.status == 400


def test_create_with_source_recipe(authed_api_client):
    """Test creating items with source recipe info."""

    client, user_id = authed_api_client
    api = ShoppingListApi(client)
    recipes_api = RecipesApi(client)

    # Create a real recipe to reference
    recipe = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Chocolate Cake",
            instructions="Bake it",
            ingredients=[make_ingredient(item="sugar")],
        )
    )

    api.create_items(
        CreateShoppingListRequest(
            items=[
                CreateShoppingListItemRequest(
                    item="sugar",
                    amount="1/2 cup",
                    source_recipe_id=str(recipe.id),
                    source_recipe_title="Chocolate Cake",
                )
            ]
        )
    )

    list_response = api.list_items()
    assert list_response.items[0].source_recipe_title == "Chocolate Cake"
    assert list_response.items[0].source_recipe_id == recipe.id


def test_update_shopping_list_item(authed_api_client):
    """Test updating a shopping list item."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    create_response = api.create_items(
        CreateShoppingListRequest(
            items=[CreateShoppingListItemRequest(item="milk", amount="1 gallon")]
        )
    )

    item_id = create_response.ids[0]

    # Update the item
    api.update_item(
        item_id,
        UpdateShoppingListItemRequest(amount="2 gallons"),
    )

    list_response = api.list_items()
    assert list_response.items[0].amount == "2 gallons"


def test_update_check_item(authed_api_client):
    """Test checking/unchecking a shopping list item."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    create_response = api.create_items(
        CreateShoppingListRequest(items=[CreateShoppingListItemRequest(item="eggs")])
    )

    item_id = create_response.ids[0]

    # Check the item
    api.update_item(item_id, UpdateShoppingListItemRequest(is_checked=True))

    list_response = api.list_items()
    assert list_response.items[0].is_checked is True

    # Uncheck the item
    api.update_item(item_id, UpdateShoppingListItemRequest(is_checked=False))

    list_response = api.list_items()
    assert list_response.items[0].is_checked is False


def test_update_shopping_list_requires_auth(unauthed_api_client):
    """Test that updating shopping list requires authentication."""
    api = ShoppingListApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        api.update_item(
            str(uuid.uuid4()),
            UpdateShoppingListItemRequest(is_checked=True),
        )

    assert exc_info.value.status == 401


def test_update_shopping_list_not_found(authed_api_client):
    """Test that updating non-existent item returns 404."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    with pytest.raises(ApiException) as exc_info:
        api.update_item(
            str(uuid.uuid4()),
            UpdateShoppingListItemRequest(is_checked=True),
        )

    assert exc_info.value.status == 404


def test_delete_shopping_list_item(authed_api_client):
    """Test deleting a shopping list item."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    create_response = api.create_items(
        CreateShoppingListRequest(items=[CreateShoppingListItemRequest(item="bread")])
    )

    item_id = create_response.ids[0]
    api.delete_item(item_id)

    # Verify it's gone
    list_response = api.list_items()
    assert len(list_response.items) == 0


def test_delete_shopping_list_requires_auth(unauthed_api_client):
    """Test that deleting shopping list requires authentication."""
    api = ShoppingListApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        api.delete_item(str(uuid.uuid4()))

    assert exc_info.value.status == 401


def test_delete_shopping_list_not_found(authed_api_client):
    """Test that deleting non-existent item returns 404."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    with pytest.raises(ApiException) as exc_info:
        api.delete_item(str(uuid.uuid4()))

    assert exc_info.value.status == 404


def test_clear_checked_items(authed_api_client):
    """Test clearing all checked items."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    # Create some items
    create_response = api.create_items(
        CreateShoppingListRequest(
            items=[
                CreateShoppingListItemRequest(item="item1"),
                CreateShoppingListItemRequest(item="item2"),
                CreateShoppingListItemRequest(item="item3"),
            ]
        )
    )

    # Check first two items
    api.update_item(
        create_response.ids[0], UpdateShoppingListItemRequest(is_checked=True)
    )
    api.update_item(
        create_response.ids[1], UpdateShoppingListItemRequest(is_checked=True)
    )

    # Clear checked
    clear_response = api.clear_checked()
    assert clear_response.deleted_count == 2

    # Verify only unchecked item remains
    list_response = api.list_items()
    assert len(list_response.items) == 1
    assert list_response.items[0].item == "item3"


def test_clear_checked_requires_auth(unauthed_api_client):
    """Test that clearing checked requires authentication."""
    api = ShoppingListApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        api.clear_checked()

    assert exc_info.value.status == 401


def test_cross_user_isolation(authed_api_client, second_authed_api_client):
    """Test that users cannot see or modify each other's shopping lists."""
    client1, user_id1 = authed_api_client
    client2, user_id2 = second_authed_api_client

    api1 = ShoppingListApi(client1)
    api2 = ShoppingListApi(client2)

    # User 1 creates items
    create_response = api1.create_items(
        CreateShoppingListRequest(
            items=[CreateShoppingListItemRequest(item="private_item")]
        )
    )
    item_id = create_response.ids[0]

    # User 2 should not see it
    list_response = api2.list_items()
    assert len(list_response.items) == 0

    # User 2 should not be able to update it
    with pytest.raises(ApiException) as exc_info:
        api2.update_item(item_id, UpdateShoppingListItemRequest(is_checked=True))
    assert exc_info.value.status == 404

    # User 2 should not be able to delete it
    with pytest.raises(ApiException) as exc_info:
        api2.delete_item(item_id)
    assert exc_info.value.status == 404


def test_sync_create_items(authed_api_client):
    """Test syncing items created offline."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    client_id = str(uuid.uuid4())
    sync_response = api.sync_items(
        SyncRequest(
            creates=[
                SyncCreateItem(
                    client_id=client_id,
                    item="synced_item",
                    amount="1 lb",
                    is_checked=False,
                    sort_order=0,
                )
            ]
        )
    )

    assert len(sync_response.created) == 1
    assert str(sync_response.created[0].client_id) == client_id
    assert sync_response.created[0].server_id is not None

    # Verify item exists
    list_response = api.list_items()
    assert len(list_response.items) == 1
    assert list_response.items[0].item == "synced_item"


def test_sync_update_items(authed_api_client):
    """Test syncing item updates."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    # Create an item first
    create_response = api.create_items(
        CreateShoppingListRequest(
            items=[CreateShoppingListItemRequest(item="update_me")]
        )
    )
    item_id = create_response.ids[0]

    # Get current version
    list_response = api.list_items()
    current_version = list_response.items[0].version

    # Sync an update
    sync_response = api.sync_items(
        SyncRequest(
            updates=[
                SyncUpdateItem(
                    id=item_id,
                    is_checked=True,
                    expected_version=current_version,
                )
            ]
        )
    )

    assert len(sync_response.updated) == 1
    assert sync_response.updated[0].success is True

    # Verify item is updated
    list_response = api.list_items()
    assert list_response.items[0].is_checked is True


def test_sync_update_conflict(authed_api_client):
    """Test sync conflict detection."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    # Create an item
    create_response = api.create_items(
        CreateShoppingListRequest(
            items=[CreateShoppingListItemRequest(item="conflict_test")]
        )
    )
    item_id = create_response.ids[0]

    # Update it via normal API (bumps version)
    api.update_item(item_id, UpdateShoppingListItemRequest(is_checked=True))

    # Try to sync with old version - should fail
    sync_response = api.sync_items(
        SyncRequest(
            updates=[
                SyncUpdateItem(
                    id=item_id,
                    is_checked=False,
                    expected_version=1,  # Old version
                )
            ]
        )
    )

    assert len(sync_response.updated) == 1
    assert sync_response.updated[0].success is False


def test_sync_delete_items(authed_api_client):
    """Test syncing item deletions."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    # Create an item
    create_response = api.create_items(
        CreateShoppingListRequest(
            items=[CreateShoppingListItemRequest(item="delete_via_sync")]
        )
    )
    item_id = create_response.ids[0]

    # Sync a delete
    sync_response = api.sync_items(SyncRequest(deletes=[item_id]))

    assert item_id in sync_response.deleted

    # Verify item is gone
    list_response = api.list_items()
    assert len(list_response.items) == 0


def test_sync_returns_server_deletions(authed_api_client):
    """Test that sync returns deletions performed on the server."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    create_response = api.create_items(
        CreateShoppingListRequest(
            items=[CreateShoppingListItemRequest(item="server_deleted")]
        )
    )
    item_id = create_response.ids[0]

    # Capture a sync timestamp
    initial_sync = api.sync_items(SyncRequest())
    last_sync_at = initial_sync.sync_timestamp

    # Delete on server
    api.delete_item(item_id)

    # Sync with last_sync_at should report deletion
    sync_response = api.sync_items(SyncRequest(last_sync_at=last_sync_at))
    assert item_id in sync_response.deleted
    assert all(change.id != item_id for change in sync_response.server_changes)


def test_sync_returns_server_changes(authed_api_client):
    """Test that sync returns server-side changes."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    # Create an item via normal API
    api.create_items(
        CreateShoppingListRequest(
            items=[CreateShoppingListItemRequest(item="server_item")]
        )
    )

    # Sync with no last_sync_at should return all items
    sync_response = api.sync_items(SyncRequest())

    assert len(sync_response.server_changes) == 1
    assert sync_response.server_changes[0].item == "server_item"
    assert sync_response.sync_timestamp is not None


def test_sync_requires_auth(unauthed_api_client):
    """Test that sync requires authentication."""
    api = ShoppingListApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        api.sync_items(SyncRequest())

    assert exc_info.value.status == 401


def test_items_ordered_by_checked_then_sort_order(authed_api_client):
    """Test that items are returned with unchecked first, then by sort_order."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    # Create items
    create_response = api.create_items(
        CreateShoppingListRequest(
            items=[
                CreateShoppingListItemRequest(item="a"),
                CreateShoppingListItemRequest(item="b"),
                CreateShoppingListItemRequest(item="c"),
            ]
        )
    )

    # Check the middle one
    api.update_item(
        create_response.ids[1], UpdateShoppingListItemRequest(is_checked=True)
    )

    # Get list
    list_response = api.list_items()

    # Unchecked items should come first
    assert list_response.items[0].item == "a"
    assert list_response.items[0].is_checked is False
    assert list_response.items[1].item == "c"
    assert list_response.items[1].is_checked is False
    # Checked item last
    assert list_response.items[2].item == "b"
    assert list_response.items[2].is_checked is True


def test_version_increments_on_update(authed_api_client):
    """Test that version increments on each update."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    create_response = api.create_items(
        CreateShoppingListRequest(
            items=[CreateShoppingListItemRequest(item="version_test")]
        )
    )
    item_id = create_response.ids[0]

    # Initial version should be 1
    list_response = api.list_items()
    assert list_response.items[0].version == 1

    # Update and check version
    api.update_item(item_id, UpdateShoppingListItemRequest(is_checked=True))
    list_response = api.list_items()
    assert list_response.items[0].version == 2

    # Another update
    api.update_item(item_id, UpdateShoppingListItemRequest(is_checked=False))
    list_response = api.list_items()
    assert list_response.items[0].version == 3


def test_items_have_category(authed_api_client):
    """Test that items include computed category based on ingredient name."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    # Create items with known categories
    api.create_items(
        CreateShoppingListRequest(
            items=[
                CreateShoppingListItemRequest(item="chicken breast"),
                CreateShoppingListItemRequest(item="olive oil"),
                CreateShoppingListItemRequest(item="fresh basil"),
                CreateShoppingListItemRequest(item="parmesan cheese"),
                CreateShoppingListItemRequest(item="unknown_xyz_ingredient"),
            ]
        )
    )

    list_response = api.list_items()
    items_by_name = {item.item: item for item in list_response.items}

    assert items_by_name["chicken breast"].category == "Meat & Seafood"
    assert items_by_name["olive oil"].category == "Oils & Vinegars"
    assert items_by_name["fresh basil"].category == "Produce"
    assert items_by_name["parmesan cheese"].category == "Cheese"
    assert items_by_name["unknown_xyz_ingredient"].category == "Other"


def test_sync_server_changes_have_category(authed_api_client):
    """Test that sync response includes category in server_changes."""
    client, user_id = authed_api_client
    api = ShoppingListApi(client)

    # Create an item via normal API
    api.create_items(
        CreateShoppingListRequest(items=[CreateShoppingListItemRequest(item="butter")])
    )

    # Sync with no last_sync_at should return all items with category
    sync_response = api.sync_items(SyncRequest())

    assert len(sync_response.server_changes) == 1
    assert sync_response.server_changes[0].item == "butter"
    assert sync_response.server_changes[0].category == "Dairy & Eggs"
