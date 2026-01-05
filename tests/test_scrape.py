import os
import time

import pytest

from ramekin_client.api import RecipesApi, ScrapeApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import CreateScrapeRequest


FIXTURE_BASE_URL = os.environ.get("FIXTURE_BASE_URL", "http://localhost:8888")


def wait_for_job_completion(scrape_api: ScrapeApi, job_id: str, timeout: float = 10.0):
    """Poll until job reaches a terminal state (completed or failed)."""
    start = time.time()
    while time.time() - start < timeout:
        job = scrape_api.get_scrape(job_id)
        if job.status in ("completed", "failed"):
            return job
        time.sleep(0.1)
    raise TimeoutError(f"Job {job_id} did not complete within {timeout}s")


class TestScrapeSuccess:
    """Test successful scraping workflow."""

    def test_scrape_creates_recipe_from_jsonld(self, authed_api_client):
        """Test that scraping a page with JSON-LD creates a recipe."""
        client, user_id = authed_api_client
        scrape_api = ScrapeApi(client)
        recipes_api = RecipesApi(client)

        url = f"{FIXTURE_BASE_URL}/seriouseats/rice_pilaf.html"
        response = scrape_api.create_scrape(CreateScrapeRequest(url=url))

        assert response.id is not None
        assert response.status == "pending"

        job = wait_for_job_completion(scrape_api, response.id)

        assert job.status == "completed"
        assert job.recipe_id is not None
        assert job.error is None
        assert job.can_retry is False

        recipe = recipes_api.get_recipe(job.recipe_id)
        assert recipe.title is not None
        assert len(recipe.title) > 0
        assert recipe.source_url == url
        assert len(recipe.ingredients) > 0
        assert len(recipe.instructions) > 0

    def test_scrape_second_recipe_from_different_url(self, authed_api_client):
        """Test that we can scrape multiple different URLs."""
        client, user_id = authed_api_client
        scrape_api = ScrapeApi(client)

        url = f"{FIXTURE_BASE_URL}/seriouseats/cream_biscuits.html"
        response = scrape_api.create_scrape(CreateScrapeRequest(url=url))

        job = wait_for_job_completion(scrape_api, response.id)

        assert job.status == "completed"
        assert job.recipe_id is not None


class TestScrapeFailureAtScrapingStep:
    """Test failure during URL fetching (scraping step)."""

    def test_scrape_disallowed_host_rejected_immediately(self, authed_api_client):
        """Test that scraping a non-allowlisted host is rejected with 400."""
        client, user_id = authed_api_client
        scrape_api = ScrapeApi(client)

        url = "http://example.com/recipe.html"

        with pytest.raises(ApiException) as exc_info:
            scrape_api.create_scrape(CreateScrapeRequest(url=url))

        assert exc_info.value.status == 400
        assert "not allowed" in str(exc_info.value.body).lower()

    def test_scrape_nonexistent_url_fails(self, authed_api_client):
        """Test that scraping a 404 URL fails at scraping step."""
        client, user_id = authed_api_client
        scrape_api = ScrapeApi(client)

        url = f"{FIXTURE_BASE_URL}/nonexistent/page.html"
        response = scrape_api.create_scrape(CreateScrapeRequest(url=url))

        job = wait_for_job_completion(scrape_api, response.id)

        assert job.status == "failed"
        assert job.failed_at_step == "scraping"
        assert job.error is not None
        assert job.can_retry is True


class TestScrapeFailureAtParsingStep:
    """Test failure during HTML parsing (parsing step)."""

    def test_scrape_no_jsonld_fails(self, authed_api_client):
        """Test that scraping a page without JSON-LD fails at parsing step."""
        client, user_id = authed_api_client
        scrape_api = ScrapeApi(client)

        url = f"{FIXTURE_BASE_URL}/no_jsonld.html"
        response = scrape_api.create_scrape(CreateScrapeRequest(url=url))

        job = wait_for_job_completion(scrape_api, response.id)

        assert job.status == "failed"
        assert job.failed_at_step == "parsing"
        assert job.error is not None
        assert job.can_retry is True
        assert job.recipe_id is None


class TestScrapeRetry:
    """Test retry functionality."""

    def test_retry_failed_job_at_scraping_step(self, authed_api_client):
        """Test that we can retry a job that failed at scraping step."""
        client, user_id = authed_api_client
        scrape_api = ScrapeApi(client)

        # Use a 404 URL on the allowed host - this fails at scraping step
        url = f"{FIXTURE_BASE_URL}/nonexistent/page.html"
        response = scrape_api.create_scrape(CreateScrapeRequest(url=url))

        job = wait_for_job_completion(scrape_api, response.id)
        assert job.status == "failed"
        assert job.failed_at_step == "scraping"
        assert job.retry_count == 0

        retry_response = scrape_api.retry_scrape(job.id)
        assert retry_response.status in ("pending", "scraping", "failed")

        job2 = wait_for_job_completion(scrape_api, job.id)
        assert job2.status == "failed"
        assert job2.retry_count == 1

    def test_retry_failed_job_at_parsing_step(self, authed_api_client):
        """Test that we can retry a job that failed at parsing step."""
        client, user_id = authed_api_client
        scrape_api = ScrapeApi(client)

        url = f"{FIXTURE_BASE_URL}/no_jsonld.html"
        response = scrape_api.create_scrape(CreateScrapeRequest(url=url))

        job = wait_for_job_completion(scrape_api, response.id)
        assert job.status == "failed"
        assert job.failed_at_step == "parsing"
        assert job.retry_count == 0

        scrape_api.retry_scrape(job.id)

        job2 = wait_for_job_completion(scrape_api, job.id)
        assert job2.status == "failed"
        assert job2.failed_at_step == "parsing"
        assert job2.retry_count == 1

    def test_cannot_retry_completed_job(self, authed_api_client):
        """Test that we cannot retry a job that completed successfully."""
        client, user_id = authed_api_client
        scrape_api = ScrapeApi(client)

        url = f"{FIXTURE_BASE_URL}/seriouseats/rice_pilaf.html"
        response = scrape_api.create_scrape(CreateScrapeRequest(url=url))

        job = wait_for_job_completion(scrape_api, response.id)
        assert job.status == "completed"
        assert job.can_retry is False

        with pytest.raises(ApiException) as exc_info:
            scrape_api.retry_scrape(job.id)

        assert exc_info.value.status == 400

    def test_cannot_retry_pending_job(self, authed_api_client):
        """Test that we cannot retry a job that is still pending/in-progress."""
        client, user_id = authed_api_client
        scrape_api = ScrapeApi(client)

        url = f"{FIXTURE_BASE_URL}/seriouseats/rice_pilaf.html"
        response = scrape_api.create_scrape(CreateScrapeRequest(url=url))

        if response.status not in ("completed", "failed"):
            with pytest.raises(ApiException) as exc_info:
                scrape_api.retry_scrape(response.id)
            assert exc_info.value.status == 400


class TestScrapeAuth:
    """Test authentication requirements."""

    def test_create_scrape_requires_auth(self, unauthed_api_client):
        """Test that creating a scrape job requires authentication."""
        scrape_api = ScrapeApi(unauthed_api_client)

        with pytest.raises(ApiException) as exc_info:
            scrape_api.create_scrape(
                CreateScrapeRequest(
                    url=f"{FIXTURE_BASE_URL}/seriouseats/rice_pilaf.html"
                )
            )

        assert exc_info.value.status == 401

    def test_get_scrape_requires_auth(self, unauthed_api_client):
        """Test that getting a scrape job requires authentication."""
        scrape_api = ScrapeApi(unauthed_api_client)

        with pytest.raises(ApiException) as exc_info:
            scrape_api.get_scrape("00000000-0000-0000-0000-000000000000")

        assert exc_info.value.status == 401

    def test_retry_scrape_requires_auth(self, unauthed_api_client):
        """Test that retrying a scrape job requires authentication."""
        scrape_api = ScrapeApi(unauthed_api_client)

        with pytest.raises(ApiException) as exc_info:
            scrape_api.retry_scrape("00000000-0000-0000-0000-000000000000")

        assert exc_info.value.status == 401


class TestScrapeIsolation:
    """Test that scrape jobs are isolated between users."""

    def test_cannot_view_other_users_scrape_job(
        self, authed_api_client, second_authed_api_client
    ):
        """Test that users cannot view each other's scrape jobs."""
        client1, user1_id = authed_api_client
        client2, user2_id = second_authed_api_client
        scrape_api1 = ScrapeApi(client1)
        scrape_api2 = ScrapeApi(client2)

        url = f"{FIXTURE_BASE_URL}/seriouseats/rice_pilaf.html"
        response = scrape_api1.create_scrape(CreateScrapeRequest(url=url))

        job = wait_for_job_completion(scrape_api1, response.id)
        assert job.status == "completed"

        with pytest.raises(ApiException) as exc_info:
            scrape_api2.get_scrape(response.id)

        assert exc_info.value.status == 404

    def test_cannot_retry_other_users_scrape_job(
        self, authed_api_client, second_authed_api_client
    ):
        """Test that users cannot retry each other's scrape jobs."""
        client1, user1_id = authed_api_client
        client2, user2_id = second_authed_api_client
        scrape_api1 = ScrapeApi(client1)
        scrape_api2 = ScrapeApi(client2)

        url = f"{FIXTURE_BASE_URL}/no_jsonld.html"
        response = scrape_api1.create_scrape(CreateScrapeRequest(url=url))

        job = wait_for_job_completion(scrape_api1, response.id)
        assert job.status == "failed"

        with pytest.raises(ApiException) as exc_info:
            scrape_api2.retry_scrape(response.id)

        assert exc_info.value.status == 404


class TestScrapeNotFound:
    """Test 404 responses for non-existent jobs."""

    def test_get_nonexistent_job(self, authed_api_client):
        """Test that getting a non-existent job returns 404."""
        client, user_id = authed_api_client
        scrape_api = ScrapeApi(client)

        with pytest.raises(ApiException) as exc_info:
            scrape_api.get_scrape("00000000-0000-0000-0000-000000000000")

        assert exc_info.value.status == 404

    def test_retry_nonexistent_job(self, authed_api_client):
        """Test that retrying a non-existent job returns 404."""
        client, user_id = authed_api_client
        scrape_api = ScrapeApi(client)

        with pytest.raises(ApiException) as exc_info:
            scrape_api.retry_scrape("00000000-0000-0000-0000-000000000000")

        assert exc_info.value.status == 404
