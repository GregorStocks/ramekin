import pytest

from conftest import make_ingredient
from ramekin_client.api import RecipesApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import CreateRecipeRequest


def _create_recipe(recipes_api):
    """Helper to create a recipe for testing."""
    request = CreateRecipeRequest(
        title="Garlic Shrimp Pasta",
        instructions="Cook pasta. Sauté garlic, add shrimp, deglaze.",
        ingredients=[
            make_ingredient(item="shrimp", amount="1", unit="lb"),
            make_ingredient(item="garlic", amount="4", unit="cloves"),
            make_ingredient(item="linguine", amount="12", unit="oz"),
            make_ingredient(item="butter", amount="3", unit="tbsp"),
            make_ingredient(item="white wine", amount="1/2", unit="cup"),
        ],
    )
    return recipes_api.create_recipe(request).id


def test_generate_description_requires_auth(unauthed_api_client):
    """Test that generate-description requires authentication."""
    recipes_api = RecipesApi(unauthed_api_client)
    with pytest.raises(ApiException) as exc_info:
        recipes_api.generate_description("00000000-0000-0000-0000-000000000000")
    assert exc_info.value.status == 401


def test_generate_description_returns_404_for_missing_recipe(authed_api_client):
    """Test that generate-description returns 404 for nonexistent recipe."""
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    with pytest.raises(ApiException) as exc_info:
        recipes_api.generate_description("00000000-0000-0000-0000-000000000000")
    assert exc_info.value.status == 404


def test_generate_description_success(authed_api_client):
    """Test that generate-description succeeds and persists the description."""
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    recipe_id = _create_recipe(recipes_api)

    result = recipes_api.generate_description(recipe_id)
    assert result.generated_description is not None
    assert len(result.generated_description) > 0
    assert result.changed is True

    # Verify the description was persisted
    recipe = recipes_api.get_recipe(recipe_id)
    assert recipe.description == result.generated_description


def test_generate_description_idempotent(authed_api_client):
    """Test that calling generate-description twice doesn't create another version."""
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    recipe_id = _create_recipe(recipes_api)

    first = recipes_api.generate_description(recipe_id)
    assert first.changed is True

    second = recipes_api.generate_description(recipe_id)
    assert second.changed is False
