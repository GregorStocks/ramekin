"""End-to-end tests for the structured `code` field on API error responses.

Every error response carries a machine-readable `code` (alongside the
human-readable `error` message) so clients branch on the code instead of
parsing message text or guessing from the HTTP status.
"""

import json
import uuid

import pytest

from ramekin_client.api import RecipesApi, TagsApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import CreateRecipeRequest, CreateTagRequest

FAKE_ID = "00000000-0000-0000-0000-000000000000"


def error_body(exc: ApiException) -> dict:
    """Parse the JSON error body off a raised ApiException."""
    return json.loads(exc.body)


def test_error_body_has_code_and_message(authed_api_client):
    """Every error response includes both a `code` and an `error` message."""
    client, _ = authed_api_client
    recipes_api = RecipesApi(client)

    with pytest.raises(ApiException) as exc_info:
        recipes_api.get_recipe(id=FAKE_ID)

    body = error_body(exc_info.value)
    assert set(body.keys()) >= {"code", "error"}
    assert isinstance(body["code"], str)
    assert isinstance(body["error"], str) and body["error"]


def test_not_found_code(authed_api_client):
    """A missing resource returns code `not_found` (404)."""
    client, _ = authed_api_client
    recipes_api = RecipesApi(client)

    with pytest.raises(ApiException) as exc_info:
        recipes_api.get_recipe(id=FAKE_ID)

    assert exc_info.value.status == 404
    assert error_body(exc_info.value)["code"] == "not_found"


def test_invalid_request_code(authed_api_client):
    """A validation failure returns code `invalid_request` (400)."""
    client, _ = authed_api_client
    recipes_api = RecipesApi(client)

    request = CreateRecipeRequest(
        title="   ",
        instructions="Mix ingredients and cook.",
        ingredients=[],
    )

    with pytest.raises(ApiException) as exc_info:
        recipes_api.create_recipe(request)

    assert exc_info.value.status == 400
    assert error_body(exc_info.value)["code"] == "invalid_request"


def test_conflict_code(authed_api_client):
    """A duplicate resource returns code `conflict` (409)."""
    client, _ = authed_api_client
    tags_api = TagsApi(client)

    name = f"dinner-{uuid.uuid4().hex[:8]}"
    tags_api.create_tag(CreateTagRequest(name=name))

    with pytest.raises(ApiException) as exc_info:
        tags_api.create_tag(CreateTagRequest(name=name))

    assert exc_info.value.status == 409
    assert error_body(exc_info.value)["code"] == "conflict"


def test_unauthorized_code(unauthed_api_client):
    """An unauthenticated request returns code `unauthorized` (401)."""
    recipes_api = RecipesApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        recipes_api.get_recipe(id=FAKE_ID)

    assert exc_info.value.status == 401
    assert error_body(exc_info.value)["code"] == "unauthorized"
