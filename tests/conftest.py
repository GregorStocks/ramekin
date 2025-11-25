import os
import sys
import uuid

import pytest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "generated"))

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, PhotosApi, TestingApi
from ramekin_client.models import SignupRequest


@pytest.fixture
def api_config():
    return Configuration(host=os.environ.get("API_BASE_URL", "http://server:3000"))


@pytest.fixture
def unauthed_api_client(api_config):
    with ApiClient(api_config) as client:
        yield client


@pytest.fixture
def auth_api(unauthed_api_client):
    return AuthApi(unauthed_api_client)


@pytest.fixture
def testing_api(unauthed_api_client):
    return TestingApi(unauthed_api_client)


@pytest.fixture
def photos_api(unauthed_api_client):
    return PhotosApi(unauthed_api_client)


@pytest.fixture
def unique_username():
    return f"testuser_{uuid.uuid4().hex[:8]}"


@pytest.fixture
def authed_api_client(api_config, auth_api, unique_username):
    response = auth_api.signup(
        SignupRequest(username=unique_username, password="testpass123")
    )
    api_config.access_token = response.token
    with ApiClient(api_config) as client:
        yield client, response.user_id


@pytest.fixture
def second_authed_api_client(api_config, auth_api):
    """A second authenticated user for testing cross-user isolation."""
    username = f"testuser2_{uuid.uuid4().hex[:8]}"
    response = auth_api.signup(SignupRequest(username=username, password="testpass123"))
    config = Configuration(host=api_config.host)
    config.access_token = response.token
    with ApiClient(config) as client:
        yield client, response.user_id
