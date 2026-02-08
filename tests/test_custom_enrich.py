import pytest

from conftest import make_ingredient
from ramekin_client.api import EnrichApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import CustomEnrichRequest, RecipeContent


def test_custom_enrich_requires_auth(unauthed_api_client):
    api = EnrichApi(unauthed_api_client)
    recipe = RecipeContent(
        title="Test Recipe",
        instructions="Mix and cook.",
        ingredients=[make_ingredient("flour", "2", "cups")],
    )
    with pytest.raises(ApiException) as exc_info:
        api.custom_enrich_recipe(
            CustomEnrichRequest(recipe=recipe, instruction="make it vegan")
        )
    assert exc_info.value.status == 401


def test_custom_enrich_returns_recipe(authed_api_client):
    client, _user_id = authed_api_client
    api = EnrichApi(client)
    recipe = RecipeContent(
        title="Chicken Stir Fry",
        instructions="Cook chicken with vegetables in a wok.",
        ingredients=[
            make_ingredient("chicken breast", "1", "lb"),
            make_ingredient("soy sauce", "2", "tbsp"),
            make_ingredient("mixed vegetables", "2", "cups"),
        ],
        tags=["dinner"],
        servings="4",
    )
    result = api.custom_enrich_recipe(
        CustomEnrichRequest(recipe=recipe, instruction="make this vegan")
    )
    assert result.title is not None
    assert "[Modified]" in result.title
    assert result.instructions is not None
    assert result.ingredients is not None
    assert len(result.ingredients) > 0
