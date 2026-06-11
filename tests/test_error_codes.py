"""End-to-end tests for the structured `code` field on API error responses.

Every error response carries a machine-readable `code` (alongside the
human-readable `error` message) so clients branch on the code instead of
parsing message text or guessing from the HTTP status.
"""

import json
import uuid

import pytest
import requests

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


def test_extractor_rejection_is_coded(authed_api_client):
    """A framework-level rejection (malformed path param that never reaches a
    handler) still returns the structured `{code, error}` body.

    Sent as a raw request because the typed client validates the UUID before it
    would ever hit the server's `Path<Uuid>` extractor.
    """
    client, _ = authed_api_client
    config = client.configuration

    response = requests.get(
        f"{config.host}/api/recipes/not-a-uuid",
        headers={
            "Authorization": f"Bearer {config.access_token}",
            "Origin": "https://example.com",
        },
    )

    assert response.status_code == 400
    body = response.json()
    assert body["code"] == "invalid_request"
    assert body["error"]
    # Headers added by inner layers (CORS) must survive the body reshaping, or
    # browser clients can't read the structured error on a cross-origin failure.
    assert response.headers.get("access-control-allow-origin") == "*"


def test_unmatched_route_is_coded(authed_api_client):
    """A completely unmatched URL (which never reaches a layer-wrapped service)
    returns a coded 404 via the explicit fallback, not Axum's plain-text 404."""
    client, _ = authed_api_client
    config = client.configuration

    response = requests.get(
        f"{config.host}/api/does-not-exist",
        headers={"Authorization": f"Bearer {config.access_token}"},
    )

    assert response.status_code == 404
    body = response.json()
    assert body["code"] == "not_found"
    assert body["error"]


def test_method_not_allowed_is_coded(server_url):
    """A method mismatch on an existing route keeps its 405 status and Allow
    header while still carrying a structured code. `/api/auth/login` is POST-only."""
    response = requests.get(f"{server_url}/api/auth/login")

    assert response.status_code == 405
    body = response.json()
    assert body["code"] == "method_not_allowed"
    assert body["error"]
    # The method-mismatch signal (Allow header) must survive the reshaping.
    assert "allow" in {k.lower() for k in response.headers}
