import uuid
from datetime import date, timedelta

import pytest

from conftest import make_ingredient
from ramekin_client.api import MealPlansApi, RecipesApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import (
    CreateMealPlanRequest,
    CreateRecipeRequest,
    UpdateMealPlanRequest,
)


def test_list_meal_plans_empty(authed_api_client):
    """Test listing meal plans when user has none."""
    client, user_id = authed_api_client
    api = MealPlansApi(client)

    response = api.list_meal_plans()
    assert response.meal_plans == []


def test_list_meal_plans_requires_auth(unauthed_api_client):
    """Test that listing meal plans requires authentication."""
    api = MealPlansApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        api.list_meal_plans()

    assert exc_info.value.status == 401


def test_create_meal_plan_success(authed_api_client):
    """Test creating a meal plan successfully."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)
    meal_plans_api = MealPlansApi(client)

    # Create a recipe first
    recipe = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Test Recipe",
            instructions="Cook it",
            ingredients=[make_ingredient(item="food")],
        )
    )

    # Create meal plan
    today = date.today()
    response = meal_plans_api.create_meal_plan(
        CreateMealPlanRequest(
            recipe_id=recipe.id,
            meal_date=today,
            meal_type="dinner",
        )
    )

    assert response.id is not None

    # Verify it appears in list
    list_response = meal_plans_api.list_meal_plans(start_date=today, end_date=today)
    assert len(list_response.meal_plans) == 1
    assert list_response.meal_plans[0].recipe_id == recipe.id
    assert list_response.meal_plans[0].meal_type == "dinner"


def test_create_meal_plan_requires_auth(unauthed_api_client):
    """Test that creating a meal plan requires authentication."""
    api = MealPlansApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        api.create_meal_plan(
            CreateMealPlanRequest(
                recipe_id=str(uuid.uuid4()),
                meal_date=date.today(),
                meal_type="lunch",
            )
        )

    assert exc_info.value.status == 401


def test_create_meal_plan_invalid_recipe(authed_api_client):
    """Test that creating meal plan with non-existent recipe fails."""
    client, user_id = authed_api_client
    api = MealPlansApi(client)

    with pytest.raises(ApiException) as exc_info:
        api.create_meal_plan(
            CreateMealPlanRequest(
                recipe_id=str(uuid.uuid4()),
                meal_date=date.today(),
                meal_type="lunch",
            )
        )

    assert exc_info.value.status == 400


def test_create_meal_plan_duplicate_fails(authed_api_client):
    """Test that creating duplicate meal plan fails with 409."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)
    meal_plans_api = MealPlansApi(client)

    recipe = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Duplicate Test",
            instructions="Cook it",
            ingredients=[make_ingredient(item="food")],
        )
    )

    today = date.today()
    meal_plans_api.create_meal_plan(
        CreateMealPlanRequest(
            recipe_id=recipe.id,
            meal_date=today,
            meal_type="breakfast",
        )
    )

    with pytest.raises(ApiException) as exc_info:
        meal_plans_api.create_meal_plan(
            CreateMealPlanRequest(
                recipe_id=recipe.id,
                meal_date=today,
                meal_type="breakfast",
            )
        )

    assert exc_info.value.status == 409


def test_create_meal_plan_multiple_recipes_same_slot(authed_api_client):
    """Test that multiple different recipes can be added to the same meal slot."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)
    meal_plans_api = MealPlansApi(client)

    recipe1 = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Main Dish",
            instructions="Cook it",
            ingredients=[make_ingredient(item="food")],
        )
    )
    recipe2 = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Side Dish",
            instructions="Prep it",
            ingredients=[make_ingredient(item="veggies")],
        )
    )

    today = date.today()

    # Both should succeed
    meal_plans_api.create_meal_plan(
        CreateMealPlanRequest(
            recipe_id=recipe1.id,
            meal_date=today,
            meal_type="dinner",
        )
    )
    meal_plans_api.create_meal_plan(
        CreateMealPlanRequest(
            recipe_id=recipe2.id,
            meal_date=today,
            meal_type="dinner",
        )
    )

    # Verify both appear
    list_response = meal_plans_api.list_meal_plans(start_date=today, end_date=today)
    assert len(list_response.meal_plans) == 2


def test_create_meal_plan_with_notes(authed_api_client):
    """Test creating a meal plan with notes."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)
    meal_plans_api = MealPlansApi(client)

    recipe = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Notes Test",
            instructions="Cook it",
            ingredients=[make_ingredient(item="food")],
        )
    )

    today = date.today()
    meal_plans_api.create_meal_plan(
        CreateMealPlanRequest(
            recipe_id=recipe.id,
            meal_date=today,
            meal_type="lunch",
            notes="Make double batch",
        )
    )

    list_response = meal_plans_api.list_meal_plans(start_date=today, end_date=today)
    assert list_response.meal_plans[0].notes == "Make double batch"


def test_delete_meal_plan_success(authed_api_client):
    """Test deleting a meal plan."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)
    meal_plans_api = MealPlansApi(client)

    recipe = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Delete Test",
            instructions="Cook it",
            ingredients=[make_ingredient(item="food")],
        )
    )

    today = date.today()
    create_response = meal_plans_api.create_meal_plan(
        CreateMealPlanRequest(
            recipe_id=recipe.id,
            meal_date=today,
            meal_type="snack",
        )
    )

    meal_plans_api.delete_meal_plan(create_response.id)

    # Verify it's gone
    list_response = meal_plans_api.list_meal_plans(start_date=today, end_date=today)
    assert len(list_response.meal_plans) == 0


def test_delete_meal_plan_requires_auth(unauthed_api_client):
    """Test that deleting a meal plan requires authentication."""
    api = MealPlansApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        api.delete_meal_plan(str(uuid.uuid4()))

    assert exc_info.value.status == 401


def test_delete_meal_plan_not_found(authed_api_client):
    """Test that deleting a non-existent meal plan returns 404."""
    client, user_id = authed_api_client
    api = MealPlansApi(client)

    with pytest.raises(ApiException) as exc_info:
        api.delete_meal_plan(str(uuid.uuid4()))

    assert exc_info.value.status == 404


def test_update_meal_plan_move_date(authed_api_client):
    """Test moving a meal plan to a different date."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)
    meal_plans_api = MealPlansApi(client)

    recipe = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Move Test",
            instructions="Cook it",
            ingredients=[make_ingredient(item="food")],
        )
    )

    today = date.today()
    tomorrow = today + timedelta(days=1)

    create_response = meal_plans_api.create_meal_plan(
        CreateMealPlanRequest(
            recipe_id=recipe.id,
            meal_date=today,
            meal_type="dinner",
        )
    )

    meal_plans_api.update_meal_plan(
        create_response.id,
        UpdateMealPlanRequest(meal_date=tomorrow),
    )

    # Verify it moved
    today_response = meal_plans_api.list_meal_plans(start_date=today, end_date=today)
    assert len(today_response.meal_plans) == 0

    tomorrow_response = meal_plans_api.list_meal_plans(
        start_date=tomorrow, end_date=tomorrow
    )
    assert len(tomorrow_response.meal_plans) == 1


def test_update_meal_plan_change_meal_type(authed_api_client):
    """Test changing a meal plan's meal type."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)
    meal_plans_api = MealPlansApi(client)

    recipe = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Type Change Test",
            instructions="Cook it",
            ingredients=[make_ingredient(item="food")],
        )
    )

    today = date.today()
    create_response = meal_plans_api.create_meal_plan(
        CreateMealPlanRequest(
            recipe_id=recipe.id,
            meal_date=today,
            meal_type="lunch",
        )
    )

    meal_plans_api.update_meal_plan(
        create_response.id,
        UpdateMealPlanRequest(meal_type="dinner"),
    )

    list_response = meal_plans_api.list_meal_plans(start_date=today, end_date=today)
    assert list_response.meal_plans[0].meal_type == "dinner"


def test_update_meal_plan_update_notes(authed_api_client):
    """Test updating meal plan notes."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)
    meal_plans_api = MealPlansApi(client)

    recipe = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Notes Update Test",
            instructions="Cook it",
            ingredients=[make_ingredient(item="food")],
        )
    )

    today = date.today()
    create_response = meal_plans_api.create_meal_plan(
        CreateMealPlanRequest(
            recipe_id=recipe.id,
            meal_date=today,
            meal_type="breakfast",
        )
    )

    meal_plans_api.update_meal_plan(
        create_response.id,
        UpdateMealPlanRequest(notes="Updated notes"),
    )

    list_response = meal_plans_api.list_meal_plans(start_date=today, end_date=today)
    assert list_response.meal_plans[0].notes == "Updated notes"


def test_update_meal_plan_requires_auth(unauthed_api_client):
    """Test that updating a meal plan requires authentication."""
    api = MealPlansApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        api.update_meal_plan(
            str(uuid.uuid4()),
            UpdateMealPlanRequest(notes="test"),
        )

    assert exc_info.value.status == 401


def test_update_meal_plan_not_found(authed_api_client):
    """Test that updating a non-existent meal plan returns 404."""
    client, user_id = authed_api_client
    api = MealPlansApi(client)

    with pytest.raises(ApiException) as exc_info:
        api.update_meal_plan(
            str(uuid.uuid4()),
            UpdateMealPlanRequest(notes="test"),
        )

    assert exc_info.value.status == 404


def test_cross_user_isolation(authed_api_client, second_authed_api_client):
    """Test that users cannot see or modify each other's meal plans."""
    client1, user_id1 = authed_api_client
    client2, user_id2 = second_authed_api_client

    recipes_api1 = RecipesApi(client1)
    meal_plans_api1 = MealPlansApi(client1)
    meal_plans_api2 = MealPlansApi(client2)

    recipe = recipes_api1.create_recipe(
        CreateRecipeRequest(
            title="Private Recipe",
            instructions="Cook it",
            ingredients=[make_ingredient(item="food")],
        )
    )

    today = date.today()
    create_response = meal_plans_api1.create_meal_plan(
        CreateMealPlanRequest(
            recipe_id=recipe.id,
            meal_date=today,
            meal_type="lunch",
        )
    )

    # User 2 should not see it
    list_response = meal_plans_api2.list_meal_plans(start_date=today, end_date=today)
    assert len(list_response.meal_plans) == 0

    # User 2 should not be able to delete it
    with pytest.raises(ApiException) as exc_info:
        meal_plans_api2.delete_meal_plan(create_response.id)
    assert exc_info.value.status == 404

    # User 2 should not be able to update it
    with pytest.raises(ApiException) as exc_info:
        meal_plans_api2.update_meal_plan(
            create_response.id,
            UpdateMealPlanRequest(notes="hacked"),
        )
    assert exc_info.value.status == 404


def test_list_meal_plans_date_range(authed_api_client):
    """Test filtering meal plans by date range."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)
    meal_plans_api = MealPlansApi(client)

    recipe = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Range Test",
            instructions="Cook it",
            ingredients=[make_ingredient(item="food")],
        )
    )

    today = date.today()
    yesterday = today - timedelta(days=1)
    tomorrow = today + timedelta(days=1)

    for d in [yesterday, today, tomorrow]:
        meal_plans_api.create_meal_plan(
            CreateMealPlanRequest(
                recipe_id=recipe.id,
                meal_date=d,
                meal_type="dinner",
            )
        )

    # Query just today
    response = meal_plans_api.list_meal_plans(start_date=today, end_date=today)
    assert len(response.meal_plans) == 1

    # Query yesterday to today
    response = meal_plans_api.list_meal_plans(start_date=yesterday, end_date=today)
    assert len(response.meal_plans) == 2

    # Query all three days
    response = meal_plans_api.list_meal_plans(start_date=yesterday, end_date=tomorrow)
    assert len(response.meal_plans) == 3


def test_list_includes_recipe_details(authed_api_client):
    """Test that listed meal plans include recipe title and thumbnail."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)
    meal_plans_api = MealPlansApi(client)

    recipe = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Detailed Recipe",
            instructions="Cook it",
            ingredients=[make_ingredient(item="food")],
        )
    )

    today = date.today()
    meal_plans_api.create_meal_plan(
        CreateMealPlanRequest(
            recipe_id=recipe.id,
            meal_date=today,
            meal_type="breakfast",
        )
    )

    response = meal_plans_api.list_meal_plans(start_date=today, end_date=today)
    assert response.meal_plans[0].recipe_title == "Detailed Recipe"


def test_meal_plan_with_deleted_recipe_not_shown(authed_api_client):
    """Test that meal plans with deleted recipes are not shown."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)
    meal_plans_api = MealPlansApi(client)

    recipe = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Will Be Deleted",
            instructions="Cook it",
            ingredients=[make_ingredient(item="food")],
        )
    )

    today = date.today()
    meal_plans_api.create_meal_plan(
        CreateMealPlanRequest(
            recipe_id=recipe.id,
            meal_date=today,
            meal_type="dinner",
        )
    )

    # Delete the recipe
    recipes_api.delete_recipe(recipe.id)

    # Meal plan should not appear (recipe is deleted)
    response = meal_plans_api.list_meal_plans(start_date=today, end_date=today)
    assert len(response.meal_plans) == 0


def test_all_meal_types(authed_api_client):
    """Test that all meal types work correctly."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)
    meal_plans_api = MealPlansApi(client)

    recipe = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="All Types Test",
            instructions="Cook it",
            ingredients=[make_ingredient(item="food")],
        )
    )

    today = date.today()
    for meal_type in ["breakfast", "lunch", "dinner", "snack"]:
        meal_plans_api.create_meal_plan(
            CreateMealPlanRequest(
                recipe_id=recipe.id,
                meal_date=today,
                meal_type=meal_type,
            )
        )

    response = meal_plans_api.list_meal_plans(start_date=today, end_date=today)
    meal_types = {mp.meal_type for mp in response.meal_plans}
    assert meal_types == {"breakfast", "lunch", "dinner", "snack"}
