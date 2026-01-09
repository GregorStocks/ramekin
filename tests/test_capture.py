import os
import time

import pytest
import requests

from ramekin_client.api import RecipesApi, ScrapeApi
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


def wait_for_job_completion(scrape_api: ScrapeApi, job_id: str, timeout: float = 10.0):
    """Poll until job completes or fails."""
    start = time.time()
    while time.time() - start < timeout:
        job = scrape_api.get_scrape(job_id)
        if job.status == "completed":
            return job
        if job.status == "failed":
            raise Exception(f"Job failed: {job.error}")
        time.sleep(0.1)
    raise TimeoutError(f"Job {job_id} did not complete within {timeout}s")


class TestCaptureSuccess:
    """Test successful HTML capture workflow."""

    def test_capture_creates_job_and_recipe(self, authed_api_client, server_url):
        """Test that capturing HTML creates a job that produces a recipe."""
        client, user_id = authed_api_client
        recipes_api = RecipesApi(client)
        scrape_api = ScrapeApi(client)
        token = client.configuration.access_token

        html = load_fixture("seriouseats/rice_pilaf.html")
        source_url = "https://www.seriouseats.com/rice-pilaf"

        # Capture returns a job ID, not recipe ID
        result = capture_recipe(server_url, token, html, source_url)
        assert "id" in result
        assert "status" in result

        # Wait for job to complete
        job = wait_for_job_completion(scrape_api, result["id"])
        assert job.recipe_id is not None

        # Verify the recipe was actually created
        recipe = recipes_api.get_recipe(job.recipe_id)
        assert recipe.source_url == source_url
        assert len(recipe.ingredients) > 0
        assert len(recipe.instructions) > 0

    def test_capture_job_starts_in_parsing_status(self, authed_api_client, server_url):
        """Test that capture jobs start in parsing status (skipping fetch)."""
        client, user_id = authed_api_client
        token = client.configuration.access_token

        html = load_fixture("seriouseats/cream_biscuits.html")
        source_url = "https://www.seriouseats.com/cream-biscuits"

        result = capture_recipe(server_url, token, html, source_url)

        # Job should start in parsing status since HTML is already provided
        assert result["status"] == "parsing"


class TestCaptureFailure:
    """Test capture failure cases."""

    def test_capture_fails_without_jsonld(self, authed_api_client, server_url):
        """Test that capturing HTML without JSON-LD results in a failed job."""
        client, user_id = authed_api_client
        scrape_api = ScrapeApi(client)
        token = client.configuration.access_token

        html = load_fixture("no_jsonld.html")
        source_url = "https://example.com/no-recipe"

        # Capture creates a job
        result = capture_recipe(server_url, token, html, source_url)
        assert "id" in result

        # Wait for job - it should fail
        with pytest.raises(Exception) as exc_info:
            wait_for_job_completion(scrape_api, result["id"])

        assert "failed" in str(exc_info.value).lower()

    def test_capture_fails_with_invalid_url(self, authed_api_client, server_url):
        """Test that capturing with invalid URL returns 400 immediately."""
        client, user_id = authed_api_client
        token = client.configuration.access_token

        html = load_fixture("seriouseats/rice_pilaf.html")
        source_url = "not-a-valid-url"

        response = capture_recipe_raw(server_url, token, html, source_url)

        assert response.status_code == 400
        body = response.json()
        assert "error" in body

    def test_capture_fails_with_empty_html(self, authed_api_client, server_url):
        """Test that capturing empty HTML results in a failed job."""
        client, user_id = authed_api_client
        scrape_api = ScrapeApi(client)
        token = client.configuration.access_token

        html = ""
        source_url = "https://example.com/empty"

        result = capture_recipe(server_url, token, html, source_url)
        assert "id" in result

        # Wait for job - it should fail
        with pytest.raises(Exception) as exc_info:
            wait_for_job_completion(scrape_api, result["id"])

        assert "failed" in str(exc_info.value).lower()


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
        scrape_api1 = ScrapeApi(client1)
        recipes_api2 = RecipesApi(client2)

        html = load_fixture("seriouseats/rice_pilaf.html")
        source_url = "https://www.seriouseats.com/rice-pilaf"

        result = capture_recipe(server_url, token1, html, source_url)

        # Wait for job to complete
        job = wait_for_job_completion(scrape_api1, result["id"])

        # Second user should not be able to access the recipe
        with pytest.raises(ApiException) as exc_info:
            recipes_api2.get_recipe(job.recipe_id)

        assert exc_info.value.status == 404
