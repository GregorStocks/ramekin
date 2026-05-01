import requests

from conftest import make_ingredient
from ramekin_client.api import RecipesApi
from ramekin_client.models import CreateRecipeRequest, UpdateRecipeRequest


def _auth_headers(client):
    return {"Authorization": f"Bearer {client.configuration.access_token}"}


def _sync(client, server_url, last_sync_at=None):
    params = {}
    if last_sync_at is not None:
        params["last_sync_at"] = last_sync_at
    response = requests.get(
        f"{server_url}/api/recipes/sync",
        headers=_auth_headers(client),
        params=params,
        timeout=10,
    )
    response.raise_for_status()
    return response.json()


def test_recipe_sync_initial_response_includes_active_recipes(
    authed_api_client, server_url
):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)

    created = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Initial Sync Recipe",
            description="Cached locally on first sync",
            instructions="Mix it.",
            ingredients=[make_ingredient(item="flour")],
            tags=["sync", "cache"],
        )
    )

    response = _sync(client, server_url)

    recipes_by_id = {recipe["id"]: recipe for recipe in response["recipes"]}
    assert str(created.id) in recipes_by_id
    assert recipes_by_id[str(created.id)]["title"] == "Initial Sync Recipe"
    assert set(recipes_by_id[str(created.id)]["tags"]) == {"sync", "cache"}
    assert response["deleted"] == []
    assert response["sync_timestamp"] is not None


def test_recipe_sync_returns_updates_and_deletions_since_last_sync(
    authed_api_client, server_url
):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)

    baseline = _sync(client, server_url)
    last_sync_at = baseline["sync_timestamp"]

    updated = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Before Update",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="rice")],
        )
    )
    deleted = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Delete Me",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="beans")],
        )
    )

    recipes_api.update_recipe(
        updated.id,
        UpdateRecipeRequest(title="After Update", tags=["changed"]),
    )
    recipes_api.delete_recipe(deleted.id)

    response = _sync(client, server_url, last_sync_at=last_sync_at)

    recipes_by_id = {recipe["id"]: recipe for recipe in response["recipes"]}
    assert str(updated.id) in recipes_by_id
    assert recipes_by_id[str(updated.id)]["title"] == "After Update"
    assert recipes_by_id[str(updated.id)]["tags"] == ["changed"]
    assert str(deleted.id) not in recipes_by_id
    assert str(deleted.id) in response["deleted"]


def test_recipe_sync_full_response_includes_deleted_ids(authed_api_client, server_url):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)

    deleted = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Deleted Before Full Sync",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="lentils")],
        )
    )
    recipes_api.delete_recipe(deleted.id)

    response = _sync(client, server_url)

    assert str(deleted.id) in response["deleted"]
    assert str(deleted.id) not in {recipe["id"] for recipe in response["recipes"]}


def test_recipe_sync_requires_auth(unauthed_api_client, server_url):
    response = requests.get(
        f"{server_url}/api/recipes/sync",
        timeout=10,
    )

    assert response.status_code == 401
