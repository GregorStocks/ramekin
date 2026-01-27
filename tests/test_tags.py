import uuid

import pytest

from ramekin_client.api import RecipesApi, TagsApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import (
    CreateRecipeRequest,
    CreateTagRequest,
    Ingredient,
    UpdateRecipeRequest,
)


def test_list_tags_empty(authed_api_client):
    """Test listing tags when user has no tags."""
    client, user_id = authed_api_client
    tags_api = TagsApi(client)

    response = tags_api.list_all_tags()
    assert response.tags == []


def test_list_tags_requires_auth(unauthed_api_client):
    """Test that listing tags requires authentication."""
    tags_api = TagsApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        tags_api.list_all_tags()

    assert exc_info.value.status == 401


def test_create_tag_success(authed_api_client):
    """Test creating a tag successfully."""
    client, user_id = authed_api_client
    tags_api = TagsApi(client)

    response = tags_api.create_tag(CreateTagRequest(name="dinner"))
    assert response.id is not None
    assert response.name == "dinner"


def test_create_tag_requires_auth(unauthed_api_client):
    """Test that creating a tag requires authentication."""
    tags_api = TagsApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        tags_api.create_tag(CreateTagRequest(name="dinner"))

    assert exc_info.value.status == 401


def test_create_tag_empty_name_fails(authed_api_client):
    """Test that creating a tag with empty name fails."""
    client, user_id = authed_api_client
    tags_api = TagsApi(client)

    with pytest.raises(ApiException) as exc_info:
        tags_api.create_tag(CreateTagRequest(name="   "))

    assert exc_info.value.status == 400


def test_create_tag_duplicate_fails(authed_api_client):
    """Test that creating a duplicate tag fails with 409."""
    client, user_id = authed_api_client
    tags_api = TagsApi(client)

    # Create first tag
    tags_api.create_tag(CreateTagRequest(name="lunch"))

    # Try to create duplicate
    with pytest.raises(ApiException) as exc_info:
        tags_api.create_tag(CreateTagRequest(name="lunch"))

    assert exc_info.value.status == 409


def test_create_tag_duplicate_case_insensitive(authed_api_client):
    """Test that tag names are case-insensitive for duplicates."""
    client, user_id = authed_api_client
    tags_api = TagsApi(client)

    # Create tag in lowercase
    tags_api.create_tag(CreateTagRequest(name="breakfast"))

    # Try to create same tag with different case
    with pytest.raises(ApiException) as exc_info:
        tags_api.create_tag(CreateTagRequest(name="BREAKFAST"))

    assert exc_info.value.status == 409


def test_delete_tag_success(authed_api_client):
    """Test deleting a tag successfully."""
    client, user_id = authed_api_client
    tags_api = TagsApi(client)

    # Create a tag
    create_response = tags_api.create_tag(CreateTagRequest(name="to-delete"))
    tag_id = create_response.id

    # Delete the tag
    tags_api.delete_tag(tag_id)

    # Verify it's gone
    response = tags_api.list_all_tags()
    assert all(t.name != "to-delete" for t in response.tags)


def test_delete_tag_requires_auth(unauthed_api_client):
    """Test that deleting a tag requires authentication."""
    tags_api = TagsApi(unauthed_api_client)

    # Try to delete any tag without auth (doesn't matter if it exists)
    fake_id = str(uuid.uuid4())
    with pytest.raises(ApiException) as exc_info:
        tags_api.delete_tag(fake_id)

    assert exc_info.value.status == 401


def test_delete_tag_not_found(authed_api_client):
    """Test that deleting a non-existent tag returns 404."""
    client, user_id = authed_api_client
    tags_api = TagsApi(client)

    fake_id = str(uuid.uuid4())

    with pytest.raises(ApiException) as exc_info:
        tags_api.delete_tag(fake_id)

    assert exc_info.value.status == 404


def test_delete_tag_cross_user_isolation(authed_api_client, second_authed_api_client):
    """Test that users cannot delete each other's tags."""
    client1, user_id1 = authed_api_client
    client2, user_id2 = second_authed_api_client

    tags_api1 = TagsApi(client1)
    tags_api2 = TagsApi(client2)

    # User 1 creates a tag
    create_response = tags_api1.create_tag(CreateTagRequest(name="private-tag"))

    # User 2 tries to delete it
    with pytest.raises(ApiException) as exc_info:
        tags_api2.delete_tag(create_response.id)

    # Should get 404 (not 403) because the tag doesn't exist for user 2
    assert exc_info.value.status == 404

    # Verify the tag still exists for user 1
    response = tags_api1.list_all_tags()
    assert any(t.name == "private-tag" for t in response.tags)


def test_delete_tag_removes_from_recipes(authed_api_client):
    """Test that deleting a tag removes it from all recipes (CASCADE)."""
    client, user_id = authed_api_client
    tags_api = TagsApi(client)
    recipes_api = RecipesApi(client)

    # Create a recipe with a tag
    recipe = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Recipe with tag",
            instructions="Cook it",
            ingredients=[Ingredient(item="food")],
            tags=["deletable-tag"],
        )
    )

    # Get the tag ID
    tags_response = tags_api.list_all_tags()
    tag = next(t for t in tags_response.tags if t.name == "deletable-tag")

    # Delete the tag
    tags_api.delete_tag(tag.id)

    # Verify the recipe no longer has the tag
    recipe_response = recipes_api.get_recipe(recipe.id)
    assert "deletable-tag" not in recipe_response.tags


def test_recipe_auto_creates_tag(authed_api_client):
    """Test that creating a recipe with a new tag auto-creates the tag."""
    client, user_id = authed_api_client
    tags_api = TagsApi(client)
    recipes_api = RecipesApi(client)

    # Create a recipe with a new tag
    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Auto-tag recipe",
            instructions="Cook it",
            ingredients=[Ingredient(item="food")],
            tags=["auto-created-tag"],
        )
    )

    # Verify the tag exists in the user's tag list
    response = tags_api.list_all_tags()
    assert any(t.name == "auto-created-tag" for t in response.tags)


def test_list_tags_includes_unused_tags(authed_api_client):
    """Test that list_tags includes tags not associated with any recipe."""
    client, user_id = authed_api_client
    tags_api = TagsApi(client)

    # Create a tag without associating it with any recipe
    tags_api.create_tag(CreateTagRequest(name="unused-tag"))

    # Verify it appears in the list
    response = tags_api.list_all_tags()
    assert any(t.name == "unused-tag" for t in response.tags)


def test_tags_returned_alphabetically(authed_api_client):
    """Test that tags are returned in alphabetical order."""
    client, user_id = authed_api_client
    tags_api = TagsApi(client)

    # Create tags out of order
    tags_api.create_tag(CreateTagRequest(name="zebra"))
    tags_api.create_tag(CreateTagRequest(name="apple"))
    tags_api.create_tag(CreateTagRequest(name="mango"))

    response = tags_api.list_all_tags()
    names = [t.name for t in response.tags]
    assert names == sorted(names, key=str.lower)


def test_recipe_tags_endpoint_returns_all_tags(authed_api_client):
    """Test that /api/recipes/tags also returns all user tags (not just used ones)."""
    client, user_id = authed_api_client
    tags_api = TagsApi(client)
    recipes_api = RecipesApi(client)

    # Create an unused tag
    tags_api.create_tag(CreateTagRequest(name="recipe-endpoint-unused"))

    # Verify it appears in the recipes tags endpoint too
    response = recipes_api.list_tags()
    assert "recipe-endpoint-unused" in response.tags


def test_update_recipe_creates_new_tags(authed_api_client):
    """Test that updating a recipe with new tags auto-creates them."""
    client, user_id = authed_api_client
    tags_api = TagsApi(client)
    recipes_api = RecipesApi(client)

    # Create a recipe
    recipe = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Update tag recipe",
            instructions="Cook it",
            ingredients=[Ingredient(item="food")],
            tags=[],
        )
    )

    # Update with a new tag
    recipes_api.update_recipe(
        recipe.id,
        UpdateRecipeRequest(
            title="Update tag recipe",
            instructions="Cook it",
            ingredients=[Ingredient(item="food")],
            tags=["new-update-tag"],
        ),
    )

    # Verify the tag exists
    response = tags_api.list_all_tags()
    assert any(t.name == "new-update-tag" for t in response.tags)
