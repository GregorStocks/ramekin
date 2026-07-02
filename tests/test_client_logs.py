"""E2E tests for the /api/client-logs endpoints."""

import pytest
from ramekin_client.api import ClientLogsApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import CreateClientLogRequest


def test_client_log_round_trip(authed_api_client):
    client, _user_id = authed_api_client
    api = ClientLogsApi(client)

    first = api.create_client_log(
        CreateClientLogRequest(
            platform="ios",
            app_version="1.0.0",
            os_info="iOS 19.0",
            content="line one\nline two\n",
        )
    )
    second = api.create_client_log(
        CreateClientLogRequest(platform="web", content="web log line\n")
    )

    listing = api.list_client_logs()
    # Newest first
    assert [u.id for u in listing.uploads] == [second.id, first.id]

    summary = listing.uploads[1]
    assert summary.platform == "ios"
    assert summary.app_version == "1.0.0"
    assert summary.os_info == "iOS 19.0"
    assert summary.content_length == len("line one\nline two\n")

    fetched = api.get_client_log(first.id)
    assert fetched.content == "line one\nline two\n"
    assert fetched.platform == "ios"


def test_client_log_user_scoping(authed_api_client, second_authed_api_client):
    client, _user_id = authed_api_client
    other_client, _other_user_id = second_authed_api_client

    created = ClientLogsApi(client).create_client_log(
        CreateClientLogRequest(platform="web", content="private logs")
    )

    other_api = ClientLogsApi(other_client)
    assert other_api.list_client_logs().uploads == []
    with pytest.raises(ApiException) as exc_info:
        other_api.get_client_log(created.id)
    assert exc_info.value.status == 404


def test_client_log_requires_auth(unauthed_api_client):
    api = ClientLogsApi(unauthed_api_client)
    with pytest.raises(ApiException) as exc_info:
        api.list_client_logs()
    assert exc_info.value.status == 401
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
