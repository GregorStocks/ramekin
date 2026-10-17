import pytest

from conftest import make_ingredient
from ramekin_client.api import RecipesApi, SyncApi, TagsApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import CreateRecipeRequest, RenameTagRequest


def tag_named(client, name):
    return next(tag for tag in TagsApi(client).list_all_tags().tags if tag.name == name)


def tag_recipe(client, *tags):
    RecipesApi(client).create_recipe(
        CreateRecipeRequest(
            title="Tagged",
            instructions="Cook it.",
            ingredients=[make_ingredient(item="salt")],
            tags=list(tags),
        )
    )


def test_tag_sync_requires_auth(unauthed_api_client):
    with pytest.raises(ApiException) as exc_info:
        SyncApi(unauthed_api_client).sync_tags()
    assert exc_info.value.status == 401


def test_full_sync_returns_every_live_tag(authed_api_client):
    client, _ = authed_api_client
    tag_recipe(client, "dinner", "quick", "gone")
    TagsApi(client).delete_tag(tag_named(client, "gone").id)

    response = SyncApi(client).sync_tags()

    assert [tag.name for tag in response.changes] == ["dinner", "quick"]
    assert response.deleted == []
    assert response.full_resync is True


def test_delta_returns_created_renamed_and_deleted_tags(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    tags_api = TagsApi(client)
    tag_recipe(client, "untouched", "renamed", "deleted")
    cursor = api.sync_tags().cursor

    tags_api.rename_tag(tag_named(client, "renamed").id, RenameTagRequest(name="new"))
    deleted = tag_named(client, "deleted").id
    tags_api.delete_tag(deleted)
    tag_recipe(client, "created")
    response = api.sync_tags(cursor=cursor)

    assert sorted(tag.name for tag in response.changes) == ["created", "new"]
    assert response.deleted == [deleted]
    assert response.full_resync is False
    assert api.sync_tags(cursor=response.cursor).changes == []


def test_a_foreign_cursor_gets_a_full_resync(authed_api_client):
    client, _ = authed_api_client
    api = SyncApi(client)
    tag_recipe(client, "dinner")
    current = api.sync_tags().cursor

    response = api.sync_tags(cursor=current + 1_000_000_000)

    assert [tag.name for tag in response.changes] == ["dinner"]
    assert response.full_resync is True


def test_tags_are_per_user(authed_api_client, second_authed_api_client):
    client1, _ = authed_api_client
    client2, _ = second_authed_api_client
    tag_recipe(client1, "private")

    assert SyncApi(client2).sync_tags().changes == []
