"""End-to-end tests for the extended scrape status response and step output endpoint."""

from datetime import datetime, timezone

import requests

from conftest import wait_for_job_completion
from ramekin_client.api import ScrapeApi
from ramekin_client.models import CreateScrapeRequest


CANONICAL_STEP_NAMES = [
    "fetch_html",
    "extract_recipe",
    "fetch_images",
    "parse_ingredients",
    "save_recipe",
    "enrich_normalize_ingredients",
    "enrich_auto_tag",
    "apply_auto_tags",
    "enrich_generate_photo",
]


class TestScrapeStatusResponse:
    def test_steps_field_present_and_ordered(self, authed_api_client, fixture_base_url):
        client, _user_id = authed_api_client
        scrape_api = ScrapeApi(client)

        url = f"{fixture_base_url}/seriouseats/rice_pilaf.html"
        created = scrape_api.create_scrape(CreateScrapeRequest(url=url))
        job = wait_for_job_completion(scrape_api, created.id)

        names = [s.name for s in job.steps]
        assert names == CANONICAL_STEP_NAMES, f"expected canonical order, got {names}"

    def test_completed_steps_have_output_and_duration(
        self, authed_api_client, fixture_base_url
    ):
        client, _user_id = authed_api_client
        scrape_api = ScrapeApi(client)

        url = f"{fixture_base_url}/seriouseats/rice_pilaf.html"
        created = scrape_api.create_scrape(CreateScrapeRequest(url=url))
        job = wait_for_job_completion(scrape_api, created.id)

        fetch_html = next(s for s in job.steps if s.name == "fetch_html")
        assert fetch_html.status == "completed"
        assert fetch_html.has_output is True
        assert fetch_html.duration_ms is not None
        assert fetch_html.finished_at is not None
        assert fetch_html.started_at is not None

    def test_created_at_present(self, authed_api_client, fixture_base_url):
        client, _user_id = authed_api_client
        scrape_api = ScrapeApi(client)

        url = f"{fixture_base_url}/seriouseats/rice_pilaf.html"
        created = scrape_api.create_scrape(CreateScrapeRequest(url=url))
        job = scrape_api.get_scrape(created.id)
        assert job.created_at is not None
        # Sanity: created_at should be within a minute of now (catches e.g. accidentally
        # populating from a different column).
        now = datetime.now(timezone.utc)
        assert abs((now - job.created_at).total_seconds()) < 60

    def test_step_output_endpoint_returns_json(
        self, authed_api_client, server_url, fixture_base_url
    ):
        client, _user_id = authed_api_client
        scrape_api = ScrapeApi(client)

        url = f"{fixture_base_url}/seriouseats/rice_pilaf.html"
        created = scrape_api.create_scrape(CreateScrapeRequest(url=url))
        wait_for_job_completion(scrape_api, created.id)

        token = client.configuration.access_token
        resp = requests.get(
            f"{server_url}/api/scrape/{created.id}/steps/fetch_html/output",
            headers={"Authorization": f"Bearer {token}"},
        )
        assert resp.status_code == 200, resp.text
        body = resp.json()
        assert isinstance(body, dict)

    def test_step_output_endpoint_404_for_unknown_step(
        self, authed_api_client, server_url, fixture_base_url
    ):
        client, _user_id = authed_api_client
        scrape_api = ScrapeApi(client)

        url = f"{fixture_base_url}/seriouseats/rice_pilaf.html"
        created = scrape_api.create_scrape(CreateScrapeRequest(url=url))
        wait_for_job_completion(scrape_api, created.id)

        token = client.configuration.access_token
        resp = requests.get(
            f"{server_url}/api/scrape/{created.id}/steps/bogus_step/output",
            headers={"Authorization": f"Bearer {token}"},
        )
        assert resp.status_code == 404

    def test_step_output_endpoint_404_for_non_owner(
        self,
        authed_api_client,
        second_authed_api_client,
        server_url,
        fixture_base_url,
    ):
        client1, _u1 = authed_api_client
        client2, _u2 = second_authed_api_client
        scrape_api1 = ScrapeApi(client1)

        url = f"{fixture_base_url}/seriouseats/rice_pilaf.html"
        created = scrape_api1.create_scrape(CreateScrapeRequest(url=url))
        wait_for_job_completion(scrape_api1, created.id)

        token2 = client2.configuration.access_token
        resp = requests.get(
            f"{server_url}/api/scrape/{created.id}/steps/fetch_html/output",
            headers={"Authorization": f"Bearer {token2}"},
        )
        assert resp.status_code == 404
