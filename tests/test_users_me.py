import pytest

from ramekin_client.api import UsersApi
from ramekin_client.exceptions import ApiException


def test_me_returns_current_user(authed_api_client, unique_username):
    """The /api/users/me endpoint returns the authenticated user's username."""
    client, user_id = authed_api_client
    users_api = UsersApi(client)

    response = users_api.me()
    assert response.username == unique_username


def test_me_requires_auth(unauthed_api_client):
    """The /api/users/me endpoint rejects unauthenticated requests."""
    users_api = UsersApi(unauthed_api_client)

    with pytest.raises(ApiException) as exc_info:
        users_api.me()

    assert exc_info.value.status == 401
