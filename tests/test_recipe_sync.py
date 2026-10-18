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


def _sync(client, server_url, cursor=None, limit=500, after_id=None):
    params = {"limit": limit}
    if cursor is not None:
        params["cursor"] = cursor
    if after_id is not None:
        params["after_id"] = after_id
    response = requests.get(
        f"{server_url}/api/sync/recipes",
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

    recipes_by_id = {recipe["id"]: recipe for recipe in response["changes"]}
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
    assert response["cursor"] > 0
    assert response["full_resync"] is True


def test_recipe_sync_returns_updates_and_deletions_since_last_sync(
    authed_api_client, server_url
):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)

    baseline = _sync(client, server_url)
    cursor = baseline["cursor"]

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
            expected_version_id=recipes_api.get_recipe(updated.id).version_id,
            title="After Update",
            ingredients=[make_ingredient(item="brown rice", note="rinsed")],
            instructions="Simmer it.",
            notes="Use a heavy pot.",
            tags=["changed"],
        ),
    )
    recipes_api.delete_recipe(deleted.id)

    response = _sync(client, server_url, cursor=cursor)

    recipes_by_id = {recipe["id"]: recipe for recipe in response["changes"]}
    assert str(updated.id) in recipes_by_id
    assert recipes_by_id[str(updated.id)]["title"] == "After Update"
    assert recipes_by_id[str(updated.id)]["ingredients"][0]["item"] == "brown rice"
    assert recipes_by_id[str(updated.id)]["ingredients"][0]["note"] == "rinsed"
    assert recipes_by_id[str(updated.id)]["instructions"] == "Simmer it."
    assert recipes_by_id[str(updated.id)]["notes"] == "Use a heavy pot."
    assert recipes_by_id[str(updated.id)]["tags"] == ["changed"]
    assert str(deleted.id) not in recipes_by_id
    assert str(deleted.id) in response["deleted"]
    assert response["full_resync"] is False


def test_recipe_sync_full_response_sends_no_tombstones(authed_api_client, server_url):
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

    # The client drops cached recipes a full resync omits instead.
    assert response["deleted"] == []
    assert str(deleted.id) not in {recipe["id"] for recipe in response["changes"]}


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
    cursor = baseline["cursor"]

    tag = next(
        tag for tag in tags_api.list_all_tags().tags if tag.name == "before-rename"
    )
    tags_api.rename_tag(tag.id, RenameTagRequest(name="after-rename"))

    response = _sync(client, server_url, cursor=cursor)

    recipes_by_id = {recipe["id"]: recipe for recipe in response["changes"]}
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
    cursor = baseline["cursor"]

    tag = next(
        tag for tag in tags_api.list_all_tags().tags if tag.name == "delete-from-sync"
    )
    tags_api.delete_tag(tag.id)

    response = _sync(client, server_url, cursor=cursor)

    recipes_by_id = {recipe["id"]: recipe for recipe in response["changes"]}
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
    cursor = after_delete["cursor"]

    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="New Recipe Revives Tag",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="peas")],
            tags=["revived-by-create"],
        )
    )

    response = _sync(client, server_url, cursor=cursor)

    recipes_by_id = {recipe["id"]: recipe for recipe in response["changes"]}
    assert recipes_by_id[str(existing.id)]["tags"] == ["revived-by-create"]


def test_recipe_sync_paginates_by_recipe_id(authed_api_client, server_url):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    cursor = _sync(client, server_url)["cursor"]

    created_ids = set()
    for index in range(3):
        created = recipes_api.create_recipe(
            CreateRecipeRequest(
                title=f"Paged Recipe {index}",
                instructions="Cook it.",
                ingredients=[make_ingredient(item="salt")],
            )
        )
        created_ids.add(str(created.id))
    deleted = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Paged Deleted Recipe",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="pepper")],
        )
    )

    recipes_api.delete_recipe(deleted.id)

    first_page = _sync(client, server_url, cursor=cursor, limit=2)

    assert len(first_page["changes"]) == 2
    assert first_page["has_more"] is True
    assert str(deleted.id) in first_page["deleted"]

    second_page = _sync(
        client,
        server_url,
        cursor=cursor,
        limit=2,
        after_id=first_page["changes"][-1]["id"],
    )

    assert second_page["has_more"] is False
    # Deletions all ride on the sweep's first page.
    assert second_page["deleted"] == []

    swept_ids = {recipe["id"] for recipe in first_page["changes"]} | {
        recipe["id"] for recipe in second_page["changes"]
    }
    assert created_ids <= swept_ids
    # Pages sweep the recipe-id space in ascending order without overlap.
    all_ids = [
        recipe["id"] for recipe in first_page["changes"] + second_page["changes"]
    ]
    assert all_ids == sorted(all_ids)
    assert len(all_ids) == len(set(all_ids))


def test_recipe_sync_first_page_cursor_covers_later_changes(
    authed_api_client, server_url
):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)

    for index in range(2):
        recipes_api.create_recipe(
            CreateRecipeRequest(
                title=f"Sweep Recipe {index}",
                instructions="Cook it.",
                ingredients=[make_ingredient(item="salt")],
            )
        )

    first_page = _sync(client, server_url, limit=1)
    assert first_page["has_more"] is True

    # A change that lands while the sweep is still paging. Persisting the
    # first page's cursor (the documented client protocol) must hand it to
    # the next sync.
    late = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Late Arrival",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="saffron")],
        )
    )
    _sync(client, server_url, limit=1, after_id=first_page["changes"][-1]["id"])

    next_sync = _sync(client, server_url, cursor=first_page["cursor"])

    assert str(late.id) in {recipe["id"] for recipe in next_sync["changes"]}


def test_recipe_sync_rejects_missing_or_invalid_limit(authed_api_client, server_url):
    client, _user_id = authed_api_client

    missing = requests.get(
        f"{server_url}/api/sync/recipes",
        headers=_auth_headers(client),
        timeout=10,
    )
    assert missing.status_code == 400

    zero = requests.get(
        f"{server_url}/api/sync/recipes",
        headers=_auth_headers(client),
        params={"limit": 0},
        timeout=10,
    )
    assert zero.status_code == 400
    assert zero.json()["code"] == "invalid_request"

    oversized = requests.get(
        f"{server_url}/api/sync/recipes",
        headers=_auth_headers(client),
        params={"limit": 501},
        timeout=10,
    )
    assert oversized.status_code == 400
    assert oversized.json()["code"] == "invalid_request"


def test_recipe_sync_requires_auth(unauthed_api_client, server_url):
    response = requests.get(
        f"{server_url}/api/sync/recipes",
        timeout=10,
    )

    assert response.status_code == 401


def test_recipe_sync_answers_a_cursor_from_another_database_with_a_full_sync(
    authed_api_client, server_url
):
    """A cursor above any this database has issued (a client that synced
    before a restore from an older backup) can't say what the client has
    seen, so the server sends every live recipe rather than silently
    skipping every change below the client's cursor."""
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    live = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Live Before Restore",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="rice")],
        )
    )
    deleted = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Deleted Before Restore",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="beans")],
        )
    )
    recipes_api.delete_recipe(deleted.id)
    current = _sync(client, server_url)["cursor"]

    response = _sync(client, server_url, cursor=current + 1_000_000_000)

    assert str(live.id) in {recipe["id"] for recipe in response["changes"]}
    assert str(deleted.id) not in {recipe["id"] for recipe in response["changes"]}
    assert response["deleted"] == []
    # Other users' writes can move the clock on in between.
    assert response["cursor"] >= current
    # Flagged so the client drops cached recipes this database doesn't have.
    assert response["full_resync"] is True


def test_recipe_sync_full_resync_flag_is_on_every_page_of_a_full_sweep(
    authed_api_client, server_url
):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    for title in ("Page One", "Page Two"):
        recipes_api.create_recipe(
            CreateRecipeRequest(
                title=title,
                instructions="Cook it.",
                ingredients=[make_ingredient(item="rice")],
            )
        )

    first = _sync(client, server_url, limit=1)
    second = _sync(
        client,
        server_url,
        limit=1,
        after_id=first["changes"][0]["id"],
    )

    assert first["has_more"] is True
    assert first["full_resync"] is True
    assert second["full_resync"] is True
