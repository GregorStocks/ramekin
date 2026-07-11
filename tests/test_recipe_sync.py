import requests

from conftest import make_ingredient
from ramekin_client.api import RecipesApi, TagsApi
from ramekin_client.models import (
    CreateRecipeRequest,
    RenameTagRequest,
    UpdateRecipeRequest,
)


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
    assert recipes_by_id[str(created.id)]["description"] == (
        "Cached locally on first sync"
    )
    assert recipes_by_id[str(created.id)]["ingredients"] == [
        {
            "item": "flour",
            "measurements": [],
            "note": None,
            "section": None,
        }
    ]
    assert recipes_by_id[str(created.id)]["instructions"] == "Mix it."
    assert recipes_by_id[str(created.id)]["notes"] is None
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
        UpdateRecipeRequest(
            title="After Update",
            ingredients=[make_ingredient(item="brown rice", note="rinsed")],
            instructions="Simmer it.",
            notes="Use a heavy pot.",
            tags=["changed"],
        ),
    )
    recipes_api.delete_recipe(deleted.id)

    response = _sync(client, server_url, last_sync_at=last_sync_at)

    recipes_by_id = {recipe["id"]: recipe for recipe in response["recipes"]}
    assert str(updated.id) in recipes_by_id
    assert recipes_by_id[str(updated.id)]["title"] == "After Update"
    assert recipes_by_id[str(updated.id)]["ingredients"][0]["item"] == "brown rice"
    assert recipes_by_id[str(updated.id)]["ingredients"][0]["note"] == "rinsed"
    assert recipes_by_id[str(updated.id)]["instructions"] == "Simmer it."
    assert recipes_by_id[str(updated.id)]["notes"] == "Use a heavy pot."
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


def test_recipe_sync_includes_recipes_after_tag_rename(authed_api_client, server_url):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    tags_api = TagsApi(client)

    created = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Tagged Recipe",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="chickpeas")],
            tags=["before-rename"],
        )
    )
    baseline = _sync(client, server_url)
    last_sync_at = baseline["sync_timestamp"]

    tag = next(
        tag for tag in tags_api.list_all_tags().tags if tag.name == "before-rename"
    )
    tags_api.rename_tag(tag.id, RenameTagRequest(name="after-rename"))

    response = _sync(client, server_url, last_sync_at=last_sync_at)

    recipes_by_id = {recipe["id"]: recipe for recipe in response["recipes"]}
    assert recipes_by_id[str(created.id)]["tags"] == ["after-rename"]


def test_recipe_sync_includes_recipes_after_tag_delete(authed_api_client, server_url):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    tags_api = TagsApi(client)

    created = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Delete Tagged Recipe",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="mushrooms")],
            tags=["delete-from-sync"],
        )
    )
    baseline = _sync(client, server_url)
    last_sync_at = baseline["sync_timestamp"]

    tag = next(
        tag for tag in tags_api.list_all_tags().tags if tag.name == "delete-from-sync"
    )
    tags_api.delete_tag(tag.id)

    response = _sync(client, server_url, last_sync_at=last_sync_at)

    recipes_by_id = {recipe["id"]: recipe for recipe in response["recipes"]}
    assert recipes_by_id[str(created.id)]["tags"] == []


def test_recipe_sync_includes_recipes_after_recipe_create_revives_tag(
    authed_api_client, server_url
):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    tags_api = TagsApi(client)

    existing = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Existing Tagged Recipe",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="barley")],
            tags=["revived-by-create"],
        )
    )
    tag = next(
        tag for tag in tags_api.list_all_tags().tags if tag.name == "revived-by-create"
    )
    tags_api.delete_tag(tag.id)
    after_delete = _sync(client, server_url)
    last_sync_at = after_delete["sync_timestamp"]

    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="New Recipe Revives Tag",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="peas")],
            tags=["revived-by-create"],
        )
    )

    response = _sync(client, server_url, last_sync_at=last_sync_at)

    recipes_by_id = {recipe["id"]: recipe for recipe in response["recipes"]}
    assert recipes_by_id[str(existing.id)]["tags"] == ["revived-by-create"]


def test_recipe_sync_requires_auth(unauthed_api_client, server_url):
    response = requests.get(
        f"{server_url}/api/recipes/sync",
        timeout=10,
    )

    assert response.status_code == 401
