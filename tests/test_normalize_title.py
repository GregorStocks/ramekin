import pytest

from conftest import make_ingredient
from ramekin_client.api import RecipesApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import CreateRecipeRequest


def test_normalize_title_requires_auth(unauthed_api_client):
    recipes_api = RecipesApi(unauthed_api_client)
    with pytest.raises(ApiException) as exc_info:
        recipes_api.normalize_title("00000000-0000-0000-0000-000000000000")
    assert exc_info.value.status == 401


def test_normalize_title_returns_404_for_missing_recipe(authed_api_client):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    with pytest.raises(ApiException) as exc_info:
        recipes_api.normalize_title("00000000-0000-0000-0000-000000000000")
    assert exc_info.value.status == 404


def test_normalize_title_applies_and_carries_tags_forward(authed_api_client):
    """A changed title writes a new version that keeps the recipe's tags."""
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # The mock normalizer echoes the title back stripped, so a trailing
    # space guarantees changed=True and exercises the write path.
    create_response = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Garlic Shrimp Pasta ",
            instructions="Cook pasta. Sauté garlic, add shrimp, toss.",
            ingredients=[
                make_ingredient(item="shrimp", amount="1", unit="lb"),
                make_ingredient(item="garlic", amount="4", unit="cloves"),
            ],
            tags=["dinner", "seafood"],
        )
    )
    recipe_id = str(create_response.id)

    result = recipes_api.normalize_title(recipe_id)
    assert result.changed is True
    assert result.normalized_title == "Garlic Shrimp Pasta"

    recipe = recipes_api.get_recipe(recipe_id)
    assert recipe.title == "Garlic Shrimp Pasta"
    assert recipe.version_source == "normalize_title"
    assert sorted(recipe.tags) == ["dinner", "seafood"]


def test_normalize_title_no_change_writes_no_version(authed_api_client):
    """An already-normalized title returns changed=False and keeps the version."""
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)

    create_response = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Garlic Shrimp Pasta",
            instructions="Cook pasta. Sauté garlic, add shrimp, toss.",
            ingredients=[make_ingredient(item="shrimp", amount="1", unit="lb")],
        )
    )
    recipe_id = str(create_response.id)

    result = recipes_api.normalize_title(recipe_id)
    assert result.changed is False
    assert result.normalized_title == "Garlic Shrimp Pasta"

    versions = recipes_api.list_versions(id=recipe_id)
    assert len(versions.versions) == 1
