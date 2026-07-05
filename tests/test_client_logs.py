"""E2E tests for the /api/client-logs endpoints."""

import json
from pathlib import Path

import pytest
import requests
from ramekin_client.api import ClientLogsApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import CreateClientLogRequest

CLIENT_LOG_DIR = Path("logs/test-client-logs")


def read_stored_upload(upload_id):
    path = CLIENT_LOG_DIR / f"{upload_id}.json"
    assert path.exists(), f"expected uploaded log at {path}"
    return json.loads(path.read_text())


def test_client_log_upload_writes_file(authed_api_client):
    client, user_id = authed_api_client
    api = ClientLogsApi(client)

    created = api.create_client_log(
        CreateClientLogRequest(
            platform="ios",
            app_version="1.0.0",
            os_info="iOS 19.0",
            content="line one\nline two\n",
        )
    )

    stored = read_stored_upload(created.id)
    assert stored["id"] == str(created.id)
    assert stored["user_id"] == str(user_id)
    assert stored["platform"] == "ios"
    assert stored["app_version"] == "1.0.0"
    assert stored["os_info"] == "iOS 19.0"
    assert stored["content"] == "line one\nline two\n"
    assert stored["created_at"]


def test_client_log_read_endpoints_do_not_exist(authed_api_client, server_url):
    client, _user_id = authed_api_client
    token = client.configuration.access_token

    created = ClientLogsApi(client).create_client_log(
        CreateClientLogRequest(platform="web", content="private logs")
    )

    headers = {"Authorization": f"Bearer {token}"}
    assert (
        requests.get(f"{server_url}/api/client-logs", headers=headers).status_code
        == 405
    )
    assert (
        requests.get(
            f"{server_url}/api/client-logs/{created.id}", headers=headers
        ).status_code
        == 404
    )


def test_client_log_requires_auth(unauthed_api_client):
    api = ClientLogsApi(unauthed_api_client)
    with pytest.raises(ApiException) as exc_info:
        api.create_client_log(CreateClientLogRequest(platform="web", content="x"))
    assert exc_info.value.status == 401


def test_client_log_rejects_bad_platform(authed_api_client):
    client, _user_id = authed_api_client
    with pytest.raises(ApiException) as exc_info:
        ClientLogsApi(client).create_client_log(
            CreateClientLogRequest(platform="android", content="x")
        )
    assert exc_info.value.status == 400


def test_client_log_rejects_empty_content(authed_api_client):
    client, _user_id = authed_api_client
    with pytest.raises(ApiException) as exc_info:
        ClientLogsApi(client).create_client_log(
            CreateClientLogRequest(platform="web", content="")
        )
    assert exc_info.value.status == 400


def test_client_log_rejects_oversized_content(authed_api_client):
    client, _user_id = authed_api_client
    too_big = "x" * (2 * 1024 * 1024 + 1)
    with pytest.raises(ApiException) as exc_info:
        ClientLogsApi(client).create_client_log(
            CreateClientLogRequest(platform="web", content=too_big)
        )
    assert exc_info.value.status == 413


def test_client_log_accepts_content_at_cap_with_heavy_json_escaping(authed_api_client):
    # Exactly at the 2MiB content cap, but every character is a double quote,
    # so the JSON-escaped wire size is well over 4MiB. The server's body
    # limit must be sized off the escaped worst case, not the raw cap, or
    # this legal payload gets 413'd before the in-handler check ever runs.
    client, _user_id = authed_api_client
    api = ClientLogsApi(client)
    content = '"' * (2 * 1024 * 1024)

    created = api.create_client_log(
        CreateClientLogRequest(platform="web", content=content)
    )

    stored = read_stored_upload(created.id)
    assert len(stored["content"]) == len(content), "stored content length mismatch"
