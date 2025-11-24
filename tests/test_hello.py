import pytest
from ramekin_client import ApiClient
from ramekin_client.api import AuthApi
from ramekin_client.exceptions import ApiException


def test_hello_with_valid_token(authed_api_client):
    client, user_id = authed_api_client
    auth_api = AuthApi(client)

    response = auth_api.hello()

    assert response.message is not None
    assert "Hello" in response.message


def test_hello_without_token(api_config):
    with ApiClient(api_config) as client:
        auth_api = AuthApi(client)

        with pytest.raises(ApiException) as exc_info:
            auth_api.hello()

        assert exc_info.value.status == 401


def test_hello_with_invalid_token(api_config):
    api_config.access_token = "invalid_token_here"
    with ApiClient(api_config) as client:
        auth_api = AuthApi(client)

        with pytest.raises(ApiException) as exc_info:
            auth_api.hello()

        assert exc_info.value.status == 401
