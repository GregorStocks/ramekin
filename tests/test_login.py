import pytest
from ramekin_client.models import SignupRequest, LoginRequest
from ramekin_client.exceptions import ApiException


def test_login_success(auth_api, unique_username):
    auth_api.signup(SignupRequest(username=unique_username, password="testpass123"))

    response = auth_api.login(
        LoginRequest(username=unique_username, password="testpass123")
    )

    assert response.token is not None
    assert len(response.token) > 0


def test_login_wrong_password(auth_api, unique_username):
    auth_api.signup(SignupRequest(username=unique_username, password="testpass123"))

    with pytest.raises(ApiException) as exc_info:
        auth_api.login(LoginRequest(username=unique_username, password="wrongpass"))

    assert exc_info.value.status == 401


def test_login_nonexistent_user(auth_api):
    with pytest.raises(ApiException) as exc_info:
        auth_api.login(
            LoginRequest(username="nonexistent_user_xyz", password="whatever")
        )

    assert exc_info.value.status == 401


def test_login_case_insensitive_username(auth_api, unique_username):
    auth_api.signup(SignupRequest(username=unique_username, password="testpass123"))

    response = auth_api.login(
        LoginRequest(username=unique_username.upper(), password="testpass123")
    )

    assert response.token is not None
