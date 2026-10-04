import json

import pytest

from conftest import make_ingredient
from ramekin_client.api import EnrichApi, TagsApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import CreateTagRequest, RecipeContent


def test_enrich_leaves_ingredients_unchanged(authed_api_client):
    """Gram amounts are computed when a recipe is read, so enrich adds none."""
    client, _ = authed_api_client
    ingredients = [
        make_ingredient(item="sugar", amount="2", unit="tbsp"),
        make_ingredient(item="chicken", amount="8", unit="oz"),
        make_ingredient(item="eggs", amount="6"),
    ]
    content = RecipeContent(
        title="Test Recipe",
        instructions="Mix and serve.",
        ingredients=ingredients,
    )

    result = EnrichApi(client).enrich_recipe(content)

    assert result.ingredients == ingredients


def test_enrich_returns_503_when_auto_tag_ai_response_is_invalid(authed_api_client):
    client, _ = authed_api_client
    tags_api = TagsApi(client)
    enrich_api = EnrichApi(client)

    tags_api.create_tag(CreateTagRequest(name="test-auto-tag"))

    content = RecipeContent(
        title="Force Auto Tag Failure",
        instructions="Mix and serve.",
        ingredients=[
            make_ingredient(item="sugar", amount="2", unit="tbsp"),
        ],
    )

    with pytest.raises(ApiException) as exc_info:
        enrich_api.enrich_recipe(content)

    assert exc_info.value.status == 503
    assert json.loads(exc_info.value.body)["code"] == "service_unavailable"
