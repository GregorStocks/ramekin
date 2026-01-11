import pytest

from ramekin_client.api import RecipesApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import (
    CreateRecipeRequest,
    Direction,
    Ingredient,
    SortBy,
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


def test_list_recipes_pagination_defaults(authed_api_client):
    """Test that pagination defaults work (limit=20, offset=0)."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create 30 recipes
    for i in range(30):
        recipes_api.create_recipe(
            create_recipe_request=CreateRecipeRequest(
                title=f"Recipe {i:02d}",
                instructions="Test instructions",
                ingredients=[],
            )
        )

    # List without pagination params
    response = recipes_api.list_recipes()

    # Should return only 20 (default limit)
    assert len(response.recipes) == 20
    assert response.pagination.total == 30
    assert response.pagination.limit == 20
    assert response.pagination.offset == 0


def test_list_recipes_pagination_custom(authed_api_client):
    """Test custom limit and offset."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create 50 recipes
    for i in range(50):
        recipes_api.create_recipe(
            create_recipe_request=CreateRecipeRequest(
                title=f"Recipe {i:02d}",
                instructions="Test instructions",
                ingredients=[],
            )
        )

    # Get second page with limit=10
    response = recipes_api.list_recipes(limit=10, offset=10)

    assert len(response.recipes) == 10
    assert response.pagination.total == 50
    assert response.pagination.limit == 10
    assert response.pagination.offset == 10


def test_list_recipes_pagination_max_limit(authed_api_client):
    """Test that limit is clamped to 1000."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create 10 recipes
    for i in range(10):
        recipes_api.create_recipe(
            create_recipe_request=CreateRecipeRequest(
                title=f"Recipe {i}",
                instructions="Test instructions",
                ingredients=[],
            )
        )

    # Request limit=9999
    response = recipes_api.list_recipes(limit=9999)

    # Actual limit should be clamped to 1000
    assert response.pagination.limit == 1000
    assert response.pagination.total == 10
    assert len(response.recipes) == 10  # Only 10 exist


def test_list_recipes_pagination_total_count(authed_api_client):
    """Test that total count is accurate across pages."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create 25 recipes
    for i in range(25):
        recipes_api.create_recipe(
            create_recipe_request=CreateRecipeRequest(
                title=f"Recipe {i:02d}",
                instructions="Test instructions",
                ingredients=[],
            )
        )

    # Request page 1
    page1 = recipes_api.list_recipes(limit=20, offset=0)
    assert page1.pagination.total == 25
    assert len(page1.recipes) == 20

    # Request page 2
    page2 = recipes_api.list_recipes(limit=20, offset=20)
    assert page2.pagination.total == 25  # Same total
    assert len(page2.recipes) == 5  # Only 5 remaining


def test_list_recipes_pagination_ordering(authed_api_client):
    """Test that pagination preserves order (updated_at DESC)."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create recipes sequentially
    recipe_ids = []
    for i in range(5):
        result = recipes_api.create_recipe(
            create_recipe_request=CreateRecipeRequest(
                title=f"Recipe {i}",
                instructions="Test instructions",
                ingredients=[],
            )
        )
        recipe_ids.append(result.id)

    # List all recipes
    response = recipes_api.list_recipes(limit=10)

    # Most recently created should be first (updated_at DESC)
    assert response.recipes[0].id == recipe_ids[4]
    assert response.recipes[4].id == recipe_ids[0]


# Search query tests


def test_search_by_title(authed_api_client):
    """Test searching recipes by title."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create recipes with different titles
    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Chicken Parmesan",
            instructions="Cook it",
            ingredients=[],
        )
    )
    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Beef Stew",
            instructions="Cook it",
            ingredients=[],
        )
    )
    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Grilled Chicken Salad",
            instructions="Cook it",
            ingredients=[],
        )
    )

    # Search for chicken
    response = recipes_api.list_recipes(q="chicken")
    assert len(response.recipes) == 2
    titles = {r.title for r in response.recipes}
    assert "Chicken Parmesan" in titles
    assert "Grilled Chicken Salad" in titles


def test_search_by_description(authed_api_client):
    """Test searching recipes by description."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Mystery Dish",
            description="A delicious vegetarian meal",
            instructions="Cook it",
            ingredients=[],
        )
    )
    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Another Dish",
            description="A meaty feast",
            instructions="Cook it",
            ingredients=[],
        )
    )

    response = recipes_api.list_recipes(q="vegetarian")
    assert len(response.recipes) == 1
    assert response.recipes[0].title == "Mystery Dish"


def test_search_case_insensitive(authed_api_client):
    """Test that search is case-insensitive."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="UPPERCASE RECIPE",
            instructions="Cook it",
            ingredients=[],
        )
    )

    response = recipes_api.list_recipes(q="uppercase")
    assert len(response.recipes) == 1

    response = recipes_api.list_recipes(q="UPPERCASE")
    assert len(response.recipes) == 1


def test_filter_by_single_tag(authed_api_client):
    """Test filtering by a single tag."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Quick Breakfast",
            instructions="Cook it",
            ingredients=[],
            tags=["breakfast", "quick"],
        )
    )
    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Slow Dinner",
            instructions="Cook it",
            ingredients=[],
            tags=["dinner", "slow"],
        )
    )

    response = recipes_api.list_recipes(q="tag:breakfast")
    assert len(response.recipes) == 1
    assert response.recipes[0].title == "Quick Breakfast"


def test_filter_by_multiple_tags_and_logic(authed_api_client):
    """Test filtering by multiple tags uses AND logic."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Quick Breakfast",
            instructions="Cook it",
            ingredients=[],
            tags=["breakfast", "quick"],
        )
    )
    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Slow Breakfast",
            instructions="Cook it",
            ingredients=[],
            tags=["breakfast", "slow"],
        )
    )

    # Both tags must match
    response = recipes_api.list_recipes(q="tag:breakfast tag:quick")
    assert len(response.recipes) == 1
    assert response.recipes[0].title == "Quick Breakfast"


def test_filter_by_tag_case_insensitive(authed_api_client):
    """Test that tag filtering is case-insensitive."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Mixed Case Recipe",
            instructions="Cook it",
            ingredients=[],
            tags=["Breakfast", "QUICK", "Vegetarian"],
        )
    )

    # Search with different cases should all match
    response = recipes_api.list_recipes(q="tag:breakfast")
    assert len(response.recipes) == 1

    response = recipes_api.list_recipes(q="tag:BREAKFAST")
    assert len(response.recipes) == 1

    response = recipes_api.list_recipes(q="tag:quick")
    assert len(response.recipes) == 1

    response = recipes_api.list_recipes(q="tag:vegetarian")
    assert len(response.recipes) == 1


def test_filter_by_source(authed_api_client):
    """Test filtering by source name."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="NYT Recipe",
            instructions="Cook it",
            ingredients=[],
            source_name="New York Times",
        )
    )
    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Other Recipe",
            instructions="Cook it",
            ingredients=[],
            source_name="AllRecipes",
        )
    )

    response = recipes_api.list_recipes(q="source:York")
    assert len(response.recipes) == 1
    assert response.recipes[0].title == "NYT Recipe"


def test_filter_has_photos(authed_api_client):
    """Test filtering by presence of photos."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create recipe without photos
    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="No Photos Recipe",
            instructions="Cook it",
            ingredients=[],
        )
    )

    # Note: We can't easily add photos in this test, so just test no:photos
    response = recipes_api.list_recipes(q="no:photos")
    assert len(response.recipes) == 1
    assert response.recipes[0].title == "No Photos Recipe"

    response = recipes_api.list_recipes(q="has:photos")
    assert len(response.recipes) == 0


def test_combined_search_and_filters(authed_api_client):
    """Test combining text search with filters."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Chicken Dinner",
            instructions="Cook it",
            ingredients=[],
            tags=["dinner"],
        )
    )
    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Chicken Breakfast",
            instructions="Cook it",
            ingredients=[],
            tags=["breakfast"],
        )
    )
    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Beef Dinner",
            instructions="Cook it",
            ingredients=[],
            tags=["dinner"],
        )
    )

    # Search for chicken with dinner tag
    response = recipes_api.list_recipes(q="chicken tag:dinner")
    assert len(response.recipes) == 1
    assert response.recipes[0].title == "Chicken Dinner"


def test_quoted_search_phrase(authed_api_client):
    """Test searching with quoted phrases."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Green Bean Casserole",
            instructions="Cook it",
            ingredients=[],
        )
    )
    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Green Salad with Beans",
            instructions="Cook it",
            ingredients=[],
        )
    )

    # Search for exact phrase "green bean"
    response = recipes_api.list_recipes(q='"green bean"')
    assert len(response.recipes) == 1
    assert response.recipes[0].title == "Green Bean Casserole"


def test_search_with_pagination(authed_api_client):
    """Test that search works with pagination."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create 5 chicken recipes
    for i in range(5):
        recipes_api.create_recipe(
            CreateRecipeRequest(
                title=f"Chicken Recipe {i}",
                instructions="Cook it",
                ingredients=[],
            )
        )

    # Create 3 beef recipes
    for i in range(3):
        recipes_api.create_recipe(
            CreateRecipeRequest(
                title=f"Beef Recipe {i}",
                instructions="Cook it",
                ingredients=[],
            )
        )

    # Search for chicken with limit
    response = recipes_api.list_recipes(q="chicken", limit=2)
    assert len(response.recipes) == 2
    assert response.pagination.total == 5

    # Get next page
    response = recipes_api.list_recipes(q="chicken", limit=2, offset=2)
    assert len(response.recipes) == 2
    assert response.pagination.total == 5


def test_empty_search_returns_all(authed_api_client):
    """Test that empty search returns all recipes."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    for i in range(3):
        recipes_api.create_recipe(
            CreateRecipeRequest(
                title=f"Recipe {i}",
                instructions="Cook it",
                ingredients=[],
            )
        )

    # Empty q should return all
    response = recipes_api.list_recipes(q="")
    assert len(response.recipes) == 3

    # No q should return all
    response = recipes_api.list_recipes()
    assert len(response.recipes) == 3


# Tags endpoint tests


def test_list_tags_empty(authed_api_client):
    """Test listing tags when user has no recipes."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    response = recipes_api.list_tags()
    assert response.tags == []


def test_list_tags_returns_distinct_tags(authed_api_client):
    """Test that tags endpoint returns distinct tags from all recipes."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create recipes with overlapping tags
    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Recipe 1",
            instructions="Cook it",
            ingredients=[],
            tags=["dinner", "quick", "vegetarian"],
        )
    )
    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Recipe 2",
            instructions="Cook it",
            ingredients=[],
            tags=["dinner", "slow", "meat"],
        )
    )

    response = recipes_api.list_tags()

    # Should have 5 unique tags, sorted alphabetically
    assert response.tags == ["dinner", "meat", "quick", "slow", "vegetarian"]


def test_list_tags_sorted_alphabetically(authed_api_client):
    """Test that tags are returned in alphabetical order."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Recipe",
            instructions="Cook it",
            ingredients=[],
            tags=["zebra", "apple", "mango"],
        )
    )

    response = recipes_api.list_tags()
    assert response.tags == ["apple", "mango", "zebra"]


def test_list_tags_requires_auth(unauthed_api_client):
    """Test that listing tags requires authentication."""
    recipes_api = RecipesApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        recipes_api.list_tags()

    assert exc_info.value.status == 401


def test_list_tags_only_from_own_recipes(authed_api_client, second_authed_api_client):
    """Test that tags endpoint only returns tags from user's own recipes."""
    client1, user1_id = authed_api_client
    client2, user2_id = second_authed_api_client
    recipes_api1 = RecipesApi(client1)
    recipes_api2 = RecipesApi(client2)

    # User 1 creates recipe with tags
    recipes_api1.create_recipe(
        CreateRecipeRequest(
            title="User 1 Recipe",
            instructions="Cook it",
            ingredients=[],
            tags=["user1-tag", "shared-tag"],
        )
    )

    # User 2 creates recipe with different tags
    recipes_api2.create_recipe(
        CreateRecipeRequest(
            title="User 2 Recipe",
            instructions="Cook it",
            ingredients=[],
            tags=["user2-tag", "shared-tag"],
        )
    )

    # User 1 should only see their tags
    response1 = recipes_api1.list_tags()
    assert "user1-tag" in response1.tags
    assert "shared-tag" in response1.tags
    assert "user2-tag" not in response1.tags

    # User 2 should only see their tags
    response2 = recipes_api2.list_tags()
    assert "user2-tag" in response2.tags
    assert "shared-tag" in response2.tags
    assert "user1-tag" not in response2.tags


def test_list_recipes_random_order(authed_api_client):
    """Test that order=random returns recipes in random order."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create 10 recipes
    for i in range(10):
        recipes_api.create_recipe(
            CreateRecipeRequest(
                title=f"Recipe {i:02d}",
                instructions="Test instructions",
                ingredients=[],
            )
        )

    # Get recipes with random order multiple times
    # and check that we get valid results (just verifying it works, not randomness)
    response = recipes_api.list_recipes(sort_by="random", limit=1)
    assert len(response.recipes) == 1
    assert response.pagination.total == 10

    # Get another random recipe
    response2 = recipes_api.list_recipes(sort_by="random", limit=1)
    assert len(response2.recipes) == 1
    assert response2.pagination.total == 10


def test_list_recipes_direction_asc_vs_desc(authed_api_client):
    """Test that dir=asc and dir=desc return opposite orders."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create recipes
    for i in range(5):
        recipes_api.create_recipe(
            CreateRecipeRequest(
                title=f"Recipe {i}",
                instructions="Test instructions",
                ingredients=[],
            )
        )

    # Get recipes in ascending and descending order (explicitly set sort_by=updated_at)
    asc_response = recipes_api.list_recipes(
        sort_by=SortBy.UPDATED_AT, sort_dir=Direction.ASC, limit=10
    )
    desc_response = recipes_api.list_recipes(
        sort_by=SortBy.UPDATED_AT, sort_dir=Direction.DESC, limit=10
    )

    # Both should return all recipes
    assert len(asc_response.recipes) == 5
    assert len(desc_response.recipes) == 5

    # Check that timestamps are actually different and in order
    asc_times = [r.updated_at for r in asc_response.recipes]
    desc_times = [r.updated_at for r in desc_response.recipes]

    # ASC should have oldest first (times increasing)
    assert asc_times == sorted(asc_times), f"ASC not sorted: {asc_times}"
    # DESC should have newest first (times decreasing)
    assert desc_times == sorted(desc_times, reverse=True), (
        f"DESC not sorted: {desc_times}"
    )


def test_create_recipe_with_paprika_fields(authed_api_client):
    """Test creating a recipe with Paprika-compatible fields."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    request = CreateRecipeRequest(
        title="Paprika Test Recipe",
        instructions="Cook the thing.",
        ingredients=[Ingredient(item="chicken", amount="1", unit="lb")],
        servings="4 servings",
        prep_time="15 mins",
        cook_time="30 mins",
        total_time="45 mins",
        rating=4,
        difficulty="Medium",
        nutritional_info="200 calories per serving",
        notes="Chef's tip: use fresh herbs.",
    )

    response = recipes_api.create_recipe(request)
    assert response.id is not None

    # Fetch the recipe and verify fields
    recipe = recipes_api.get_recipe(str(response.id))
    assert recipe.title == "Paprika Test Recipe"
    assert recipe.servings == "4 servings"
    assert recipe.prep_time == "15 mins"
    assert recipe.cook_time == "30 mins"
    assert recipe.total_time == "45 mins"
    assert recipe.rating == 4
    assert recipe.difficulty == "Medium"
    assert recipe.nutritional_info == "200 calories per serving"
    assert recipe.notes == "Chef's tip: use fresh herbs."


def test_update_recipe_paprika_fields(authed_api_client):
    """Test updating Paprika-compatible fields on a recipe."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create recipe without paprika fields
    create_response = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Update Test",
            instructions="Original instructions",
            ingredients=[],
        )
    )

    # Update with paprika fields
    recipes_api.update_recipe(
        str(create_response.id),
        UpdateRecipeRequest(
            servings="2 servings",
            rating=5,
            notes="Updated notes",
        ),
    )

    # Verify updates
    recipe = recipes_api.get_recipe(str(create_response.id))
    assert recipe.servings == "2 servings"
    assert recipe.rating == 5
    assert recipe.notes == "Updated notes"


def test_recipe_paprika_fields_optional(authed_api_client):
    """Test that Paprika fields are optional and default to None."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create minimal recipe
    response = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Minimal Recipe",
            instructions="Just cook it.",
            ingredients=[],
        )
    )

    # Verify optional fields are None
    recipe = recipes_api.get_recipe(str(response.id))
    assert recipe.servings is None
    assert recipe.prep_time is None
    assert recipe.cook_time is None
    assert recipe.total_time is None
    assert recipe.rating is None
    assert recipe.difficulty is None
    assert recipe.nutritional_info is None
    assert recipe.notes is None


# Version tests


def test_recipe_has_version_info_on_create(authed_api_client):
    """Test that a newly created recipe has version info."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    response = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Versioned Recipe",
            instructions="Do the thing",
            ingredients=[],
        )
    )

    recipe = recipes_api.get_recipe(str(response.id))

    # Should have version metadata
    assert recipe.version_id is not None
    assert recipe.version_source == "user"


def test_update_recipe_creates_new_version(authed_api_client):
    """Test that updating a recipe creates a new version."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create a recipe
    create_response = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Original Title",
            instructions="Original instructions",
            ingredients=[],
        )
    )
    recipe_id = str(create_response.id)

    # Get original version info
    original_recipe = recipes_api.get_recipe(recipe_id)
    original_version_id = original_recipe.version_id

    # Update the recipe
    recipes_api.update_recipe(
        recipe_id,
        UpdateRecipeRequest(title="Updated Title"),
    )

    # Get updated recipe
    updated_recipe = recipes_api.get_recipe(recipe_id)

    # Version ID should be different
    assert updated_recipe.version_id != original_version_id
    assert updated_recipe.title == "Updated Title"
    assert updated_recipe.version_source == "user"


def test_list_versions(authed_api_client):
    """Test listing versions of a recipe."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create a recipe
    create_response = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Version Test Recipe",
            instructions="Instructions",
            ingredients=[],
        )
    )
    recipe_id = str(create_response.id)

    # Update it twice
    recipes_api.update_recipe(
        recipe_id,
        UpdateRecipeRequest(title="Version 2"),
    )
    recipes_api.update_recipe(
        recipe_id,
        UpdateRecipeRequest(title="Version 3"),
    )

    # List versions
    versions = recipes_api.list_versions(recipe_id)

    # Should have 3 versions
    assert len(versions.versions) == 3

    # Most recent should be first (newest first)
    assert versions.versions[0].title == "Version 3"
    assert versions.versions[0].is_current is True
    assert versions.versions[1].title == "Version 2"
    assert versions.versions[1].is_current is False
    assert versions.versions[2].title == "Version Test Recipe"
    assert versions.versions[2].is_current is False

    # All should have version_source
    for v in versions.versions:
        assert v.version_source == "user"


def test_get_specific_version(authed_api_client):
    """Test getting a specific version by version_id."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create a recipe
    create_response = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Original Title",
            instructions="Original instructions",
            ingredients=[Ingredient(item="original ingredient")],
        )
    )
    recipe_id = str(create_response.id)

    # Get original version ID
    original_recipe = recipes_api.get_recipe(recipe_id)
    original_version_id = str(original_recipe.version_id)

    # Update the recipe
    recipes_api.update_recipe(
        recipe_id,
        UpdateRecipeRequest(
            title="Updated Title",
            ingredients=[Ingredient(item="new ingredient")],
        ),
    )

    # Get current version (default)
    current = recipes_api.get_recipe(recipe_id)
    assert current.title == "Updated Title"
    assert current.ingredients[0].item == "new ingredient"

    # Get original version by version_id
    original = recipes_api.get_recipe(recipe_id, version_id=original_version_id)
    assert original.title == "Original Title"
    assert original.ingredients[0].item == "original ingredient"


def test_list_versions_requires_auth(unauthed_api_client):
    """Test that listing versions requires authentication."""
    recipes_api = RecipesApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        recipes_api.list_versions("00000000-0000-0000-0000-000000000000")

    assert exc_info.value.status == 401


def test_list_versions_not_found(authed_api_client):
    """Test that listing versions of non-existent recipe returns 404."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    with pytest.raises(ApiException) as exc_info:
        recipes_api.list_versions("00000000-0000-0000-0000-000000000000")

    assert exc_info.value.status == 404


def test_get_version_not_found(authed_api_client):
    """Test that getting non-existent version returns 404."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create a recipe
    create_response = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Test Recipe",
            instructions="Instructions",
            ingredients=[],
        )
    )
    recipe_id = str(create_response.id)

    # Try to get with a fake version ID
    with pytest.raises(ApiException) as exc_info:
        recipes_api.get_recipe(
            recipe_id, version_id="00000000-0000-0000-0000-000000000000"
        )

    assert exc_info.value.status == 404


def test_cannot_access_other_users_versions(
    authed_api_client, second_authed_api_client
):
    """Test that users cannot access versions of other users' recipes."""
    client1, user1_id = authed_api_client
    client2, user2_id = second_authed_api_client
    recipes_api1 = RecipesApi(client1)
    recipes_api2 = RecipesApi(client2)

    # User 1 creates a recipe
    create_response = recipes_api1.create_recipe(
        CreateRecipeRequest(
            title="User 1 Recipe",
            instructions="Instructions",
            ingredients=[],
        )
    )
    recipe_id = str(create_response.id)

    # User 2 should not be able to list versions
    with pytest.raises(ApiException) as exc_info:
        recipes_api2.list_versions(recipe_id)
    assert exc_info.value.status == 404
