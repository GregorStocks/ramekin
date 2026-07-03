import pytest
import threading
import time

from conftest import make_ingredient
from ramekin_client.api import RecipesApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import CreateRecipeRequest, UpdateRecipeRequest


def test_generate_photo_requires_auth(unauthed_api_client):
    recipes_api = RecipesApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        recipes_api.generate_photo(id="00000000-0000-0000-0000-000000000000")

    assert exc_info.value.status == 401


def test_generate_photo_creates_new_version_and_photo(authed_api_client, query_tracker):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)

    create_response = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Roasted Tomato Pasta",
            description="Jammy roasted tomatoes with garlic and basil.",
            instructions="Roast the tomatoes, boil pasta, and toss together.",
            ingredients=[
                make_ingredient("tomatoes", "2", "lb"),
                make_ingredient("garlic", "4", "cloves"),
                make_ingredient("basil", "1", "cup"),
            ],
            tags=["dinner"],
        )
    )

    response = recipes_api.generate_photo_with_http_info(id=str(create_response.id))
    query_tracker.record(
        "POST",
        f"{client.configuration.host}/api/recipes/{create_response.id}/generate-photo",
        dict(response.headers),
    )

    generated = response.data
    recipe = recipes_api.get_recipe(id=str(create_response.id))
    versions = recipes_api.list_versions(id=str(create_response.id))

    assert generated.photo_id is not None
    assert generated.version_id is not None
    assert recipe.version_source == "ai_photo"
    assert recipe.tags == ["dinner"]
    assert len(recipe.photo_ids) == 1
    assert str(recipe.photo_ids[0]) == str(generated.photo_id)
    assert versions.versions[0].version_source == "ai_photo"


def test_generate_photo_rejects_other_users_recipe(
    authed_api_client, second_authed_api_client
):
    client, _user_id = authed_api_client
    other_client, _other_user_id = second_authed_api_client
    recipes_api = RecipesApi(client)
    other_recipes_api = RecipesApi(other_client)

    create_response = other_recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Secret Recipe",
            instructions="Keep it secret.",
            ingredients=[make_ingredient("salt", "1", "pinch")],
        )
    )

    with pytest.raises(ApiException) as exc_info:
        recipes_api.generate_photo(id=str(create_response.id))

    assert exc_info.value.status == 404


def test_generate_photo_fails_if_recipe_changes_mid_generation(authed_api_client):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)

    create_response = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Slow Generated Photo",
            description="A recipe used to simulate photo generation races.",
            instructions="Toast the bread and serve it warm.",
            ingredients=[
                make_ingredient("bread", "2", "slices"),
                make_ingredient("butter", "1", "tbsp"),
            ],
        )
    )

    result: dict[str, object] = {}

    def generate_photo() -> None:
        try:
            recipes_api.generate_photo(id=str(create_response.id))
            result["status"] = "ok"
        except ApiException as exc:
            result["status"] = "error"
            result["code"] = exc.status
            result["body"] = exc.body

    thread = threading.Thread(target=generate_photo)
    thread.start()

    time.sleep(0.2)
    recipes_api.update_recipe(
        id=str(create_response.id),
        update_recipe_request=UpdateRecipeRequest(
            title="Updated While Generating",
            description="A recipe used to simulate photo generation races.",
            instructions="Toast the bread and serve it warm.",
            ingredients=[
                make_ingredient("bread", "2", "slices"),
                make_ingredient("butter", "1", "tbsp"),
            ],
        ),
    )

    thread.join(timeout=5)

    assert result["status"] == "error"
    assert result["code"] == 409
