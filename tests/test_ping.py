import pytest
from ramekin_client import ApiClient
from ramekin_client.api import TestApi
from ramekin_client.exceptions import ApiException


def test_ping_with_valid_token(authed_api_client):
    client, user_id = authed_api_client
    test_api = TestApi(client)

    response = test_api.ping()

    assert response.message == "ping"


def test_ping_without_token(api_config):
    with ApiClient(api_config) as client:
        test_api = TestApi(client)

        with pytest.raises(ApiException) as exc_info:
            test_api.ping()

        assert exc_info.value.status == 401


def test_ping_with_invalid_token(api_config):
    api_config.access_token = "invalid_token_here"
    with ApiClient(api_config) as client:
        test_api = TestApi(client)

        with pytest.raises(ApiException) as exc_info:
            test_api.ping()

        assert exc_info.value.status == 401
