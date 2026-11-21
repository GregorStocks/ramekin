import requests

from conftest import connect_db, make_ingredient
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


def test_recipe_sync_redelivers_a_recipe_whose_calorie_estimate_changed(
    authed_api_client, server_url, database_url
):
    """The server rewrites a version's estimate in place as ingredient names
    resolve. Local calorie filters read the synced value, so the change has
    to reach the next delta."""
    client, _user_id = authed_api_client
    created = RecipesApi(client).create_recipe(
        CreateRecipeRequest(
            title="Estimated Later",
            instructions="Mix it.",
            ingredients=[make_ingredient(item="flour")],
        )
    )
    cursor = _sync(client, server_url)["cursor"]
    assert _sync(client, server_url, cursor=cursor)["changes"] == []

    conn = connect_db(database_url)
    try:
        conn.execute(
            """
            update recipe_versions
            set kcal_per_serving = 432.5, kcal_per_serving_label = '~430 kcal'
            where id = (select current_version_id from recipes where id = ?)
            """,
            (str(created.id),),
        )
    finally:
        conn.close()

    delta = _sync(client, server_url, cursor=cursor)
    [changed] = delta["changes"]
    assert changed["id"] == str(created.id)
    assert changed["kcal_per_serving"] == 432.5
    assert changed["calories_per_serving"] == "~430 kcal"

    # Rewriting the same estimate is not a change.
    cursor = delta["cursor"]
    conn = connect_db(database_url)
    try:
        conn.execute(
            """
            update recipe_versions
            set kcal_per_serving = 432.5, kcal_per_serving_label = '~430 kcal'
            where id = (select current_version_id from recipes where id = ?)
            """,
            (str(created.id),),
        )
    finally:
        conn.close()
    assert _sync(client, server_url, cursor=cursor)["changes"] == []


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


def _create(recipes_api, title, tags=None):
    return recipes_api.create_recipe(
        CreateRecipeRequest(
            title=title,
            instructions="Cook it.",
            ingredients=[make_ingredient(item="salt")],
            tags=tags,
        )
    )


def _next_page(client, server_url, page, limit):
    return _sync(
        client,
        server_url,
        cursor=page["cursor"],
        after_id=page["after_id"],
        limit=limit,
    )


def test_recipe_sync_pages_in_change_order(authed_api_client, server_url):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    cursor = _sync(client, server_url)["cursor"]

    created = [str(_create(recipes_api, f"Paged {i}").id) for i in range(3)]
    deleted = _create(recipes_api, "Paged Deleted")
    recipes_api.delete_recipe(deleted.id)

    first_page = _sync(client, server_url, cursor=cursor, limit=2)

    assert [recipe["id"] for recipe in first_page["changes"]] == created[:2]
    assert first_page["has_more"] is True
    assert first_page["after_id"] == created[1]
    assert first_page["full_resync"] is False
    assert str(deleted.id) in first_page["deleted"]

    second_page = _next_page(client, server_url, first_page, limit=2)

    assert [recipe["id"] for recipe in second_page["changes"]] == created[2:]
    assert second_page["has_more"] is False
    assert second_page["after_id"] is None
    assert second_page["full_resync"] is False
    # The delete came after the first page's position, so it rides along
    # again: a client that only applied this page still learns of it.
    assert str(deleted.id) in second_page["deleted"]

    # The final cursor is past everything delivered.
    caught_up = _sync(client, server_url, cursor=second_page["cursor"])
    assert caught_up["changes"] == []
    assert caught_up["deleted"] == []


def test_recipe_sync_delivers_changes_that_land_while_paging(
    authed_api_client, server_url
):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    early = _create(recipes_api, "Early")
    _create(recipes_api, "Middle")

    page = _sync(client, server_url, limit=1)
    assert page["full_resync"] is True
    assert page["has_more"] is True
    assert [recipe["id"] for recipe in page["changes"]] == [str(early.id)]

    # Both land behind the position the client has already paged past in
    # recipe-id terms, but ahead of it in the change stream.
    recipes_api.update_recipe(
        early.id,
        UpdateRecipeRequest(
            title="Early, Edited",
            expected_version_id=recipes_api.get_recipe(early.id).version_id,
        ),
    )
    late = _create(recipes_api, "Late Arrival")

    titles = {}
    while page["has_more"]:
        page = _next_page(client, server_url, page, limit=1)
        # Only the page that restarted the stream is a full resync.
        assert page["full_resync"] is False
        for recipe in page["changes"]:
            titles[recipe["id"]] = recipe["title"]

    assert titles[str(early.id)] == "Early, Edited"
    assert titles[str(late.id)] == "Late Arrival"
    assert page["after_id"] is None


def test_recipe_sync_pages_through_recipes_sharing_one_change_stamp(
    authed_api_client, server_url
):
    """A tag rename stamps the tag, so every recipe carrying it sits at the
    same place in the stream; `after_id` is what tells them apart."""
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    tagged = {
        str(_create(recipes_api, f"Tagged {i}", tags=["Shared"]).id) for i in range(3)
    }
    cursor = _sync(client, server_url)["cursor"]

    tags_api = TagsApi(client)
    tag = next(tag for tag in tags_api.list_all_tags().tags if tag.name == "Shared")
    tags_api.rename_tag(tag.id, RenameTagRequest(name="Shared Renamed"))

    page = _sync(client, server_url, cursor=cursor, limit=1)
    pages = [page]
    while page["has_more"]:
        page = _next_page(client, server_url, page, limit=1)
        pages.append(page)

    delivered = [recipe for page in pages for recipe in page["changes"]]
    assert sorted(recipe["id"] for recipe in delivered) == sorted(tagged)
    assert all(recipe["tags"] == ["Shared Renamed"] for recipe in delivered)
    # All three share the stamp: the cursor holds still while after_id moves.
    assert pages[0]["cursor"] == pages[1]["cursor"]
    assert pages[0]["after_id"] != pages[1]["after_id"]


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
