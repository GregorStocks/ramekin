import pytest

from conftest import make_ingredient
from ramekin_client.api import EnrichApi, PhotosApi
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


def test_custom_enrich_uses_recipe_photos(authed_api_client, test_image):
    client, _user_id = authed_api_client
    enrich_api = EnrichApi(client)
    photos_api = PhotosApi(client)
    photo_id = photos_api.upload(file=("test.png", test_image)).id

    recipe = RecipeContent(
        title="Chicken Stir Fry",
        instructions="Cook chicken with vegetables in a wok.",
        ingredients=[
            make_ingredient("chicken breast", "1", "lb"),
            make_ingredient("soy sauce", "2", "tbsp"),
        ],
    )

    result = enrich_api.custom_enrich_recipe(
        CustomEnrichRequest(
            recipe=recipe,
            instruction="make this spicier",
            photo_ids=[photo_id],
        )
    )

    assert result.title is not None
    assert "[Modified with Photo]" in result.title


def test_custom_enrich_allows_duplicate_photo_ids(authed_api_client, test_image):
    client, _user_id = authed_api_client
    enrich_api = EnrichApi(client)
    photos_api = PhotosApi(client)
    photo_id = photos_api.upload(file=("test.png", test_image)).id

    recipe = RecipeContent(
        title="Test Recipe",
        instructions="Mix and cook.",
        ingredients=[make_ingredient("flour", "2", "cups")],
    )

    result = enrich_api.custom_enrich_recipe(
        CustomEnrichRequest(
            recipe=recipe,
            instruction="make it crispier",
            photo_ids=[photo_id, photo_id],
        )
    )

    assert result.title is not None
    assert "[Modified with Photo]" in result.title


def test_custom_enrich_rejects_other_users_photos(
    authed_api_client, second_authed_api_client, test_image
):
    client, _user_id = authed_api_client
    other_client, _other_user_id = second_authed_api_client
    enrich_api = EnrichApi(client)
    other_photos_api = PhotosApi(other_client)
    other_photo_id = other_photos_api.upload(file=("test.png", test_image)).id

    recipe = RecipeContent(
        title="Test Recipe",
        instructions="Mix and cook.",
        ingredients=[make_ingredient("flour", "2", "cups")],
    )

    with pytest.raises(ApiException) as exc_info:
        enrich_api.custom_enrich_recipe(
            CustomEnrichRequest(
                recipe=recipe,
                instruction="make it vegan",
                photo_ids=[other_photo_id],
            )
        )

    assert exc_info.value.status == 400
