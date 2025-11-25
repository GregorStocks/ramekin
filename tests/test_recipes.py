import pytest

from ramekin_client.api import RecipesApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import (
    CreateRecipeRequest,
    Ingredient,
    UpdateRecipeRequest,
)


def test_list_recipes_empty(authed_api_client):
    """Test listing recipes when user has no recipes."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    response = recipes_api.list_recipes()
    assert response.recipes == []


def test_list_recipes_requires_auth(unauthed_api_client):
    """Test that listing recipes requires authentication."""
    recipes_api = RecipesApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        recipes_api.list_recipes()

    assert exc_info.value.status == 401


def test_create_recipe_success(authed_api_client):
    """Test creating a recipe successfully."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    request = CreateRecipeRequest(
        title="Test Recipe",
        instructions="Mix ingredients and cook.",
        ingredients=[
            Ingredient(item="flour", amount="2", unit="cups"),
            Ingredient(item="sugar", amount="1", unit="cup"),
        ],
        description="A test recipe",
        tags=["test", "easy"],
    )

    response = recipes_api.create_recipe(request)
    assert response.id is not None


def test_create_recipe_requires_auth(unauthed_api_client):
    """Test that creating a recipe requires authentication."""
    recipes_api = RecipesApi(unauthed_api_client)

    request = CreateRecipeRequest(
        title="Test Recipe",
        instructions="Mix ingredients and cook.",
        ingredients=[],
    )

    with pytest.raises(ApiException) as exc_info:
        recipes_api.create_recipe(request)

    assert exc_info.value.status == 401


def test_create_recipe_empty_title_fails(authed_api_client):
    """Test that creating a recipe with empty title fails."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    request = CreateRecipeRequest(
        title="   ",
        instructions="Mix ingredients and cook.",
        ingredients=[],
    )

    with pytest.raises(ApiException) as exc_info:
        recipes_api.create_recipe(request)

    assert exc_info.value.status == 400


def test_create_recipe_empty_instructions_fails(authed_api_client):
    """Test that creating a recipe with empty instructions fails."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    request = CreateRecipeRequest(
        title="Test Recipe",
        instructions="   ",
        ingredients=[],
    )

    with pytest.raises(ApiException) as exc_info:
        recipes_api.create_recipe(request)

    assert exc_info.value.status == 400


def test_get_recipe_success(authed_api_client):
    """Test getting a recipe by ID."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create a recipe first
    create_request = CreateRecipeRequest(
        title="Get Test Recipe",
        instructions="Step 1: Do this. Step 2: Do that.",
        ingredients=[
            Ingredient(item="eggs", amount="3"),
            Ingredient(item="milk", amount="1", unit="cup", note="whole milk"),
        ],
        description="A recipe to test get",
        source_url="https://example.com/recipe",
        source_name="Example Recipes",
        tags=["breakfast", "quick"],
    )

    create_response = recipes_api.create_recipe(create_request)
    recipe_id = str(create_response.id)

    # Get the recipe
    recipe = recipes_api.get_recipe(id=recipe_id)

    assert str(recipe.id) == recipe_id
    assert recipe.title == "Get Test Recipe"
    assert recipe.instructions == "Step 1: Do this. Step 2: Do that."
    assert recipe.description == "A recipe to test get"
    assert recipe.source_url == "https://example.com/recipe"
    assert recipe.source_name == "Example Recipes"
    assert len(recipe.ingredients) == 2
    assert recipe.ingredients[0].item == "eggs"
    assert recipe.ingredients[1].note == "whole milk"
    assert set(recipe.tags) == {"breakfast", "quick"}
    assert recipe.created_at is not None
    assert recipe.updated_at is not None


def test_get_recipe_not_found(authed_api_client):
    """Test getting a non-existent recipe returns 404."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    fake_id = "00000000-0000-0000-0000-000000000000"

    with pytest.raises(ApiException) as exc_info:
        recipes_api.get_recipe(id=fake_id)

    assert exc_info.value.status == 404


def test_get_recipe_requires_auth(unauthed_api_client):
    """Test that getting a recipe requires authentication."""
    recipes_api = RecipesApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        recipes_api.get_recipe(id="00000000-0000-0000-0000-000000000000")

    assert exc_info.value.status == 401


def test_update_recipe_success(authed_api_client):
    """Test updating a recipe."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create a recipe first
    create_request = CreateRecipeRequest(
        title="Original Title",
        instructions="Original instructions",
        ingredients=[Ingredient(item="original ingredient")],
    )

    create_response = recipes_api.create_recipe(create_request)
    recipe_id = str(create_response.id)

    # Update the recipe
    update_request = UpdateRecipeRequest(
        title="Updated Title",
        instructions="Updated instructions",
        ingredients=[
            Ingredient(item="new ingredient 1"),
            Ingredient(item="new ingredient 2"),
        ],
        description="Now with a description",
        tags=["updated"],
    )

    recipes_api.update_recipe(id=recipe_id, update_recipe_request=update_request)

    # Verify the update
    recipe = recipes_api.get_recipe(id=recipe_id)
    assert recipe.title == "Updated Title"
    assert recipe.instructions == "Updated instructions"
    assert recipe.description == "Now with a description"
    assert len(recipe.ingredients) == 2
    assert recipe.tags == ["updated"]


def test_update_recipe_partial(authed_api_client):
    """Test partially updating a recipe (only some fields)."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create a recipe first
    create_request = CreateRecipeRequest(
        title="Original Title",
        instructions="Original instructions",
        ingredients=[Ingredient(item="ingredient")],
        description="Original description",
    )

    create_response = recipes_api.create_recipe(create_request)
    recipe_id = str(create_response.id)

    # Update only the title
    update_request = UpdateRecipeRequest(title="New Title Only")

    recipes_api.update_recipe(id=recipe_id, update_recipe_request=update_request)

    # Verify only title changed
    recipe = recipes_api.get_recipe(id=recipe_id)
    assert recipe.title == "New Title Only"
    assert recipe.instructions == "Original instructions"
    assert recipe.description == "Original description"


def test_update_recipe_not_found(authed_api_client):
    """Test updating a non-existent recipe returns 404."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    fake_id = "00000000-0000-0000-0000-000000000000"
    update_request = UpdateRecipeRequest(title="New Title")

    with pytest.raises(ApiException) as exc_info:
        recipes_api.update_recipe(id=fake_id, update_recipe_request=update_request)

    assert exc_info.value.status == 404


def test_update_recipe_empty_title_fails(authed_api_client):
    """Test that updating a recipe with empty title fails."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create a recipe first
    create_request = CreateRecipeRequest(
        title="Original Title",
        instructions="Instructions",
        ingredients=[],
    )

    create_response = recipes_api.create_recipe(create_request)
    recipe_id = str(create_response.id)

    # Try to update with empty title
    update_request = UpdateRecipeRequest(title="   ")

    with pytest.raises(ApiException) as exc_info:
        recipes_api.update_recipe(id=recipe_id, update_recipe_request=update_request)

    assert exc_info.value.status == 400


def test_update_recipe_requires_auth(unauthed_api_client):
    """Test that updating a recipe requires authentication."""
    recipes_api = RecipesApi(unauthed_api_client)

    update_request = UpdateRecipeRequest(title="New Title")

    with pytest.raises(ApiException) as exc_info:
        recipes_api.update_recipe(
            id="00000000-0000-0000-0000-000000000000",
            update_recipe_request=update_request,
        )

    assert exc_info.value.status == 401


def test_delete_recipe_success(authed_api_client):
    """Test deleting a recipe."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create a recipe first
    create_request = CreateRecipeRequest(
        title="Recipe to Delete",
        instructions="Instructions",
        ingredients=[],
    )

    create_response = recipes_api.create_recipe(create_request)
    recipe_id = str(create_response.id)

    # Delete the recipe
    recipes_api.delete_recipe(id=recipe_id)

    # Verify it's gone
    with pytest.raises(ApiException) as exc_info:
        recipes_api.get_recipe(id=recipe_id)

    assert exc_info.value.status == 404


def test_delete_recipe_not_found(authed_api_client):
    """Test deleting a non-existent recipe returns 404."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    fake_id = "00000000-0000-0000-0000-000000000000"

    with pytest.raises(ApiException) as exc_info:
        recipes_api.delete_recipe(id=fake_id)

    assert exc_info.value.status == 404


def test_delete_recipe_requires_auth(unauthed_api_client):
    """Test that deleting a recipe requires authentication."""
    recipes_api = RecipesApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        recipes_api.delete_recipe(id="00000000-0000-0000-0000-000000000000")

    assert exc_info.value.status == 401


def test_list_recipes_returns_created_recipes(authed_api_client):
    """Test that list_recipes returns recipes the user has created."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create two recipes
    recipe1 = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Recipe 1",
            instructions="Instructions 1",
            ingredients=[],
            tags=["tag1"],
        )
    )
    recipe2 = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Recipe 2",
            instructions="Instructions 2",
            ingredients=[],
            description="Description 2",
        )
    )

    # List recipes
    response = recipes_api.list_recipes()
    assert len(response.recipes) == 2

    # Check that both created recipes are in the list
    recipe_ids = {str(r.id) for r in response.recipes}
    assert str(recipe1.id) in recipe_ids
    assert str(recipe2.id) in recipe_ids

    # Verify each recipe has required fields
    for recipe in response.recipes:
        assert recipe.id is not None
        assert recipe.title is not None
        assert recipe.created_at is not None
        assert recipe.updated_at is not None


def test_recipes_only_visible_to_owner(authed_api_client, second_authed_api_client):
    """Test that users can only see their own recipes."""
    client1, user1_id = authed_api_client
    client2, user2_id = second_authed_api_client
    recipes_api1 = RecipesApi(client1)
    recipes_api2 = RecipesApi(client2)

    # User 1 creates a recipe
    recipes_api1.create_recipe(
        CreateRecipeRequest(
            title="User 1 Recipe",
            instructions="Instructions",
            ingredients=[],
        )
    )

    # User 2 creates a recipe
    recipes_api2.create_recipe(
        CreateRecipeRequest(
            title="User 2 Recipe",
            instructions="Instructions",
            ingredients=[],
        )
    )

    # User 1 should only see their own recipe
    user1_recipes = recipes_api1.list_recipes()
    assert len(user1_recipes.recipes) == 1
    assert user1_recipes.recipes[0].title == "User 1 Recipe"

    # User 2 should only see their own recipe
    user2_recipes = recipes_api2.list_recipes()
    assert len(user2_recipes.recipes) == 1
    assert user2_recipes.recipes[0].title == "User 2 Recipe"


def test_cannot_access_other_users_recipe(authed_api_client, second_authed_api_client):
    """Test that a user cannot get/update/delete another user's recipe."""
    client1, user1_id = authed_api_client
    client2, user2_id = second_authed_api_client
    recipes_api1 = RecipesApi(client1)
    recipes_api2 = RecipesApi(client2)

    # User 1 creates a recipe
    create_response = recipes_api1.create_recipe(
        CreateRecipeRequest(
            title="User 1 Private Recipe",
            instructions="Secret instructions",
            ingredients=[],
        )
    )
    recipe_id = str(create_response.id)

    # User 2 should not be able to get it
    with pytest.raises(ApiException) as exc_info:
        recipes_api2.get_recipe(id=recipe_id)
    assert exc_info.value.status == 404

    # User 2 should not be able to update it
    with pytest.raises(ApiException) as exc_info:
        recipes_api2.update_recipe(
            id=recipe_id,
            update_recipe_request=UpdateRecipeRequest(title="Hacked!"),
        )
    assert exc_info.value.status == 404

    # User 2 should not be able to delete it
    with pytest.raises(ApiException) as exc_info:
        recipes_api2.delete_recipe(id=recipe_id)
    assert exc_info.value.status == 404

    # Verify it still exists for user 1
    recipe = recipes_api1.get_recipe(id=recipe_id)
    assert recipe.title == "User 1 Private Recipe"
