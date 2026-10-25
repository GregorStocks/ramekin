import pytest

from conftest import make_ingredient
from ramekin_client.api import EditRecipeApi, PhotosApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import EditRecipeRequest, RecipeContent


def test_edit_recipe_requires_auth(unauthed_api_client):
    api = EditRecipeApi(unauthed_api_client)
    recipe = RecipeContent(
        title="Test Recipe",
        instructions="Mix and cook.",
        ingredients=[make_ingredient("flour", "2", "cups")],
    )
    with pytest.raises(ApiException) as exc_info:
        api.edit_recipe_with_ai(
            EditRecipeRequest(recipe=recipe, instruction="make it vegan")
        )
    assert exc_info.value.status == 401


def test_edit_recipe_returns_recipe(authed_api_client):
    client, _user_id = authed_api_client
    api = EditRecipeApi(client)
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
    result = api.edit_recipe_with_ai(
        EditRecipeRequest(recipe=recipe, instruction="make this vegan")
    )
    assert result.title is not None
    assert "[Modified]" in result.title
    assert result.instructions is not None
    assert result.ingredients is not None
    assert len(result.ingredients) > 0


def test_edit_recipe_uses_recipe_photos(authed_api_client, test_image):
    client, _user_id = authed_api_client
    edit_api = EditRecipeApi(client)
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

    result = edit_api.edit_recipe_with_ai(
        EditRecipeRequest(
            recipe=recipe,
            instruction="make this spicier",
            photo_ids=[photo_id],
        )
    )

    assert result.title is not None
    assert "[Modified with Photo]" in result.title


def test_edit_recipe_allows_duplicate_photo_ids(authed_api_client, test_image):
    client, _user_id = authed_api_client
    edit_api = EditRecipeApi(client)
    photos_api = PhotosApi(client)
    photo_id = photos_api.upload(file=("test.png", test_image)).id

    recipe = RecipeContent(
        title="Test Recipe",
        instructions="Mix and cook.",
        ingredients=[make_ingredient("flour", "2", "cups")],
    )

    result = edit_api.edit_recipe_with_ai(
        EditRecipeRequest(
            recipe=recipe,
            instruction="make it crispier",
            photo_ids=[photo_id, photo_id],
        )
    )

    assert result.title is not None
    assert "[Modified with Photo]" in result.title


def test_edit_recipe_rejects_other_users_photos(
    authed_api_client, second_authed_api_client, test_image
):
    client, _user_id = authed_api_client
    other_client, _other_user_id = second_authed_api_client
    edit_api = EditRecipeApi(client)
    other_photos_api = PhotosApi(other_client)
    other_photo_id = other_photos_api.upload(file=("test.png", test_image)).id

    recipe = RecipeContent(
        title="Test Recipe",
        instructions="Mix and cook.",
        ingredients=[make_ingredient("flour", "2", "cups")],
    )

    with pytest.raises(ApiException) as exc_info:
        edit_api.edit_recipe_with_ai(
            EditRecipeRequest(
                recipe=recipe,
                instruction="make it vegan",
                photo_ids=[other_photo_id],
            )
        )

    assert exc_info.value.status == 400
