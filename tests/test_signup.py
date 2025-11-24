import pytest
from ramekin_client.models import SignupRequest
from ramekin_client.exceptions import ApiException


def test_signup_success(auth_api, unique_username):
    response = auth_api.signup(
        SignupRequest(username=unique_username, password="testpass123")
    )

    assert response.user_id is not None
    assert response.token is not None
    assert len(response.token) > 0


def test_signup_duplicate_username(auth_api, unique_username):
    auth_api.signup(SignupRequest(username=unique_username, password="testpass123"))

    with pytest.raises(ApiException) as exc_info:
        auth_api.signup(SignupRequest(username=unique_username, password="otherpass"))

    assert exc_info.value.status == 409


def test_signup_duplicate_username_case_insensitive(auth_api, unique_username):
    auth_api.signup(SignupRequest(username=unique_username, password="testpass123"))

    with pytest.raises(ApiException) as exc_info:
        auth_api.signup(
            SignupRequest(username=unique_username.upper(), password="otherpass")
        )

    assert exc_info.value.status == 409
