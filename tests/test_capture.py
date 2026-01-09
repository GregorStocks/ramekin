import os

import pytest
import requests

from ramekin_client.api import RecipesApi
from ramekin_client.exceptions import ApiException


def load_fixture(name: str) -> str:
    """Load an HTML fixture file."""
    fixture_path = os.path.join(os.path.dirname(__file__), "scrape_fixtures", name)
    with open(fixture_path, encoding="utf-8") as f:
        return f.read()


def capture_recipe(server_url: str, token: str, html: str, source_url: str) -> dict:
    """Call the capture endpoint directly (generated client doesn't have it yet)."""
    response = requests.post(
        f"{server_url}/api/scrape/capture",
        json={"html": html, "source_url": source_url},
        headers={"Authorization": f"Bearer {token}"},
    )
    response.raise_for_status()
    return response.json()


def capture_recipe_raw(
    server_url: str, token: str | None, html: str, source_url: str
) -> requests.Response:
    """Call the capture endpoint and return raw response."""
    headers = {}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    return requests.post(
        f"{server_url}/api/scrape/capture",
        json={"html": html, "source_url": source_url},
        headers=headers,
    )


class TestCaptureSuccess:
    """Test successful HTML capture workflow."""

    def test_capture_creates_recipe_from_html(self, authed_api_client, server_url):
        """Test that capturing HTML with JSON-LD creates a recipe."""
        client, user_id = authed_api_client
        recipes_api = RecipesApi(client)
        token = client.configuration.access_token

        html = load_fixture("seriouseats/rice_pilaf.html")
        source_url = "https://www.seriouseats.com/rice-pilaf"

        result = capture_recipe(server_url, token, html, source_url)

        assert "recipe_id" in result
        assert "title" in result
        assert len(result["title"]) > 0

        # Verify the recipe was actually created
        recipe = recipes_api.get_recipe(result["recipe_id"])
        assert recipe.title == result["title"]
        assert recipe.source_url == source_url
        assert len(recipe.ingredients) > 0
        assert len(recipe.instructions) > 0

    def test_capture_extracts_recipe_title(self, authed_api_client, server_url):
        """Test that capture returns the correct recipe title."""
        client, user_id = authed_api_client
        token = client.configuration.access_token

        html = load_fixture("seriouseats/cream_biscuits.html")
        source_url = "https://www.seriouseats.com/cream-biscuits"

        result = capture_recipe(server_url, token, html, source_url)

        # The title should be extracted from JSON-LD
        assert result["title"] is not None
        assert len(result["title"]) > 0


class TestCaptureFailure:
    """Test capture failure cases."""

    def test_capture_fails_without_jsonld(self, authed_api_client, server_url):
        """Test that capturing HTML without JSON-LD returns 400."""
        client, user_id = authed_api_client
        token = client.configuration.access_token

        html = load_fixture("no_jsonld.html")
        source_url = "https://example.com/no-recipe"

        response = capture_recipe_raw(server_url, token, html, source_url)

        assert response.status_code == 400
        body = response.json()
        assert "error" in body
        assert "recipe" in body["error"].lower() or "no" in body["error"].lower()

    def test_capture_fails_with_invalid_url(self, authed_api_client, server_url):
        """Test that capturing with invalid URL returns 400."""
        client, user_id = authed_api_client
        token = client.configuration.access_token

        html = load_fixture("seriouseats/rice_pilaf.html")
        source_url = "not-a-valid-url"

        response = capture_recipe_raw(server_url, token, html, source_url)

        assert response.status_code == 400
        body = response.json()
        assert "error" in body

    def test_capture_fails_with_empty_html(self, authed_api_client, server_url):
        """Test that capturing empty HTML returns 400."""
        client, user_id = authed_api_client
        token = client.configuration.access_token

        html = ""
        source_url = "https://example.com/empty"

        response = capture_recipe_raw(server_url, token, html, source_url)

        assert response.status_code == 400


class TestCaptureAuth:
    """Test authentication requirements for capture endpoint."""

    def test_capture_requires_auth(self, server_url):
        """Test that capture endpoint requires authentication."""
        html = load_fixture("seriouseats/rice_pilaf.html")
        source_url = "https://www.seriouseats.com/rice-pilaf"

        response = capture_recipe_raw(server_url, None, html, source_url)

        assert response.status_code == 401

    def test_capture_fails_with_invalid_token(self, server_url):
        """Test that capture endpoint fails with invalid token."""
        html = load_fixture("seriouseats/rice_pilaf.html")
        source_url = "https://www.seriouseats.com/rice-pilaf"

        response = capture_recipe_raw(server_url, "invalid-token", html, source_url)

        assert response.status_code == 401


class TestCaptureIsolation:
    """Test that captured recipes are isolated between users."""

    def test_captured_recipe_belongs_to_user(
        self, authed_api_client, second_authed_api_client, server_url
    ):
        """Test that captured recipe belongs to the capturing user."""
        client1, user1_id = authed_api_client
        client2, user2_id = second_authed_api_client
        token1 = client1.configuration.access_token
        recipes_api2 = RecipesApi(client2)

        html = load_fixture("seriouseats/rice_pilaf.html")
        source_url = "https://www.seriouseats.com/rice-pilaf"

        result = capture_recipe(server_url, token1, html, source_url)

        # Second user should not be able to access the recipe
        with pytest.raises(ApiException) as exc_info:
            recipes_api2.get_recipe(result["recipe_id"])

        assert exc_info.value.status == 404
