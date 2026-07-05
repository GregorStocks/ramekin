import os
import time
from pathlib import Path
from uuid import uuid4

import pytest

from conftest import make_ingredient
from ramekin_client.api import RecipesApi, ScrapeApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import (
    CreateRecipeRequest,
    CreateScrapeRequest,
    UpdateRecipeRequest,
)


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


class TestRescrapeSuccess:
    """Test successful rescrape workflow."""

    def test_rescrape_creates_new_version(self, authed_api_client):
        """Test that rescraping creates a rescrape version in history."""
        client, user_id = authed_api_client
        scrape_api = ScrapeApi(client)
        recipes_api = RecipesApi(client)

        # First, scrape a recipe
        url = f"{FIXTURE_BASE_URL}/seriouseats/rice_pilaf.html"
        response = scrape_api.create_scrape(CreateScrapeRequest(url=url))
        job = wait_for_job_completion(scrape_api, response.id)

        assert job.status == "completed"
        recipe_id = job.recipe_id

        # Get the original recipe
        original_recipe = recipes_api.get_recipe(recipe_id)
        original_version_id = original_recipe.version_id
        recipes_api.update_recipe(
            recipe_id,
            UpdateRecipeRequest(
                title=original_recipe.title,
                description=original_recipe.description,
                ingredients=original_recipe.ingredients,
                instructions=original_recipe.instructions,
                source_url=original_recipe.source_url,
                source_name=original_recipe.source_name,
                photo_ids=original_recipe.photo_ids,
                servings=original_recipe.servings,
                prep_time=original_recipe.prep_time,
                cook_time=original_recipe.cook_time,
                total_time=original_recipe.total_time,
                rating=original_recipe.rating,
                difficulty=original_recipe.difficulty,
                nutritional_info=original_recipe.nutritional_info,
                notes=original_recipe.notes,
                tags=["rescrape-tag", "weeknight"],
            ),
        )
        tagged_recipe = recipes_api.get_recipe(recipe_id)
        tagged_version_id = tagged_recipe.version_id

        # Rescrape the recipe
        rescrape_response = recipes_api.rescrape(recipe_id)
        assert rescrape_response.job_id is not None
        assert rescrape_response.status == "pending"

        # Wait for rescrape to complete
        rescrape_job = wait_for_job_completion(scrape_api, rescrape_response.job_id)

        assert rescrape_job.status == "completed"
        # The recipe_id should be the same (same recipe, new version)
        assert rescrape_job.recipe_id == recipe_id

        # Get the updated recipe
        updated_recipe = recipes_api.get_recipe(recipe_id)

        # Verify a new version was created
        assert tagged_version_id != original_version_id
        assert updated_recipe.version_id != tagged_version_id
        assert updated_recipe.version_source in (
            "rescrape",
            "normalize_title",
            "generate_description",
            "enrichment",
        )
        assert sorted(updated_recipe.tags) == ["rescrape-tag", "weeknight"]
        versions = recipes_api.list_versions(recipe_id)
        assert any(v.version_source == "rescrape" for v in versions.versions)
        # The recipe ID should be the same
        assert updated_recipe.id == original_recipe.id

    def test_rescrape_preserves_version_history(self, authed_api_client):
        """Test that rescrape adds to history without removing old versions."""
        client, user_id = authed_api_client
        scrape_api = ScrapeApi(client)
        recipes_api = RecipesApi(client)

        # Scrape a recipe
        url = f"{FIXTURE_BASE_URL}/seriouseats/cream_biscuits.html"
        response = scrape_api.create_scrape(CreateScrapeRequest(url=url))
        job = wait_for_job_completion(scrape_api, response.id)

        assert job.status == "completed"
        recipe_id = job.recipe_id

        # Get initial version count
        initial_versions = recipes_api.list_versions(recipe_id)
        initial_count = len(initial_versions.versions)

        # Rescrape
        rescrape_response = recipes_api.rescrape(recipe_id)
        wait_for_job_completion(scrape_api, rescrape_response.job_id)

        # Verify version count increased
        final_versions = recipes_api.list_versions(recipe_id)
        assert len(final_versions.versions) > initial_count


class TestRescrapeValidation:
    """Test rescrape validation errors."""

    def test_rescrape_requires_source_url(self, authed_api_client):
        """Test that rescraping a recipe without source_url returns 400."""
        client, user_id = authed_api_client
        recipes_api = RecipesApi(client)

        # Create a recipe manually (no source_url)
        recipe = recipes_api.create_recipe(
            CreateRecipeRequest(
                title="Manual Recipe",
                ingredients=[make_ingredient(item="test ingredient")],
                instructions="test instructions",
            )
        )

        # Try to rescrape
        with pytest.raises(ApiException) as exc_info:
            recipes_api.rescrape(recipe.id)

        assert exc_info.value.status == 400
        assert "source" in str(exc_info.value.body).lower()

    def test_rescrape_nonexistent_recipe(self, authed_api_client):
        """Test that rescraping non-existent recipe returns 404."""
        client, user_id = authed_api_client
        recipes_api = RecipesApi(client)

        with pytest.raises(ApiException) as exc_info:
            recipes_api.rescrape("00000000-0000-0000-0000-000000000000")

        assert exc_info.value.status == 404


class TestRescrapeAuth:
    """Test rescrape authentication requirements."""

    def test_rescrape_requires_auth(self, unauthed_api_client):
        """Test that rescraping requires authentication."""
        recipes_api = RecipesApi(unauthed_api_client)

        with pytest.raises(ApiException) as exc_info:
            recipes_api.rescrape("00000000-0000-0000-0000-000000000000")

        assert exc_info.value.status == 401


_MINIMAL_RECIPE_HTML = """<!DOCTYPE html>
<html>
<head>
<title>Local Photo Rescrape Recipe</title>
<script type="application/ld+json">
{{
  "@context": "https://schema.org",
  "@type": "Recipe",
  "name": "Local Photo Rescrape Recipe",
  "image": "{image_url}",
  "recipeIngredient": ["1 cup flour", "2 eggs"],
  "recipeInstructions": "Mix and bake.",
  "author": {{"@type": "Person", "name": "Test Author"}}
}}
</script>
</head>
<body><h1>Local Photo Rescrape Recipe</h1></body>
</html>
"""


def _write_local_fixture(name: str, html: str) -> tuple[Path, str]:
    """Write an HTML fixture into the fixture server's root so it can be fetched
    back by the scraper over localhost. Returns the absolute path and URL path."""
    fixtures_dir = Path(__file__).parent / "scrape_fixtures"
    unique_name = f"{Path(name).stem}-{uuid4().hex}.html"
    path = fixtures_dir / unique_name
    path.write_text(html)
    return path, unique_name


class TestRescrapePhoto:
    """Test the photo-only rescrape workflow."""

    def test_rescrape_photo_creates_version_with_same_content(self, authed_api_client):
        """Photo rescrape should make a new version whose only change is photo_ids."""
        client, _ = authed_api_client
        scrape_api = ScrapeApi(client)
        recipes_api = RecipesApi(client)

        image_url = f"{FIXTURE_BASE_URL}/images/test_recipe.jpg"
        fixture_path, fixture_name = _write_local_fixture(
            "rescrape_photo_fixture.html",
            _MINIMAL_RECIPE_HTML.format(image_url=image_url),
        )
        try:
            source_url = f"{FIXTURE_BASE_URL}/{fixture_name}"
            response = scrape_api.create_scrape(CreateScrapeRequest(url=source_url))
            job = wait_for_job_completion(scrape_api, response.id)
            assert job.status == "completed"
            recipe_id = job.recipe_id

            original = recipes_api.get_recipe(recipe_id)
            original_title = original.title
            original_instructions = original.instructions

            rescrape_response = recipes_api.rescrape_photo(recipe_id)
            assert rescrape_response.job_id is not None
            rescrape_job = wait_for_job_completion(scrape_api, rescrape_response.job_id)
            assert rescrape_job.status == "completed"
            assert rescrape_job.recipe_id == recipe_id

            updated = recipes_api.get_recipe(recipe_id)
            # A fresh version was created...
            assert updated.version_id != original.version_id
            assert updated.version_source == "photo_rescrape"
            # ...but the recipe content is preserved verbatim.
            assert updated.title == original_title
            assert updated.instructions == original_instructions
            # ...and a photo still exists (same slot replaced, not blanked).
            assert len(updated.photo_ids or []) > 0
        finally:
            fixture_path.unlink(missing_ok=True)

    def test_rescrape_photo_creates_exactly_one_new_version(self, authed_api_client):
        """Photo rescrape must not trigger the enrichment chain afterwards —
        otherwise auto-tagging would create a second 'enrichment' version and
        the endpoint would silently mutate metadata beyond photo_ids."""
        client, _ = authed_api_client
        scrape_api = ScrapeApi(client)
        recipes_api = RecipesApi(client)

        image_url = f"{FIXTURE_BASE_URL}/images/test_recipe.jpg"
        fixture_path, fixture_name = _write_local_fixture(
            "rescrape_photo_fixture_single_version.html",
            _MINIMAL_RECIPE_HTML.format(image_url=image_url),
        )
        try:
            source_url = f"{FIXTURE_BASE_URL}/{fixture_name}"
            response = scrape_api.create_scrape(CreateScrapeRequest(url=source_url))
            job = wait_for_job_completion(scrape_api, response.id)
            assert job.status == "completed"
            recipe_id = job.recipe_id

            before = recipes_api.list_versions(recipe_id)
            before_count = len(before.versions)

            rescrape_response = recipes_api.rescrape_photo(recipe_id)
            wait_for_job_completion(scrape_api, rescrape_response.job_id)

            after = recipes_api.list_versions(recipe_id)
            # Exactly one new version — the photo_rescrape one — with no
            # follow-up enrichment version from apply_auto_tags.
            assert len(after.versions) == before_count + 1
            # The newest version is photo_rescrape, not enrichment.
            newest = after.versions[0]
            assert newest.version_source == "photo_rescrape"
        finally:
            fixture_path.unlink(missing_ok=True)

    def test_rescrape_photo_fails_loudly_when_no_images_fetched(
        self, authed_api_client
    ):
        """If the source URL produces no fetchable image, photo rescrape should
        fail rather than silently blanking out the existing photo_ids."""
        client, _ = authed_api_client
        scrape_api = ScrapeApi(client)
        recipes_api = RecipesApi(client)

        # rice_pilaf's image URLs point at seriouseats.com, outside the
        # SCRAPE_ALLOWED_HOSTS allowlist — so image fetch produces zero photos.
        url = f"{FIXTURE_BASE_URL}/seriouseats/rice_pilaf.html"
        response = scrape_api.create_scrape(CreateScrapeRequest(url=url))
        job = wait_for_job_completion(scrape_api, response.id)
        assert job.status == "completed"
        recipe_id = job.recipe_id

        rescrape_response = recipes_api.rescrape_photo(recipe_id)
        rescrape_job = wait_for_job_completion(scrape_api, rescrape_response.job_id)
        assert rescrape_job.status == "failed"

    def test_rescrape_photo_requires_source_url(self, authed_api_client):
        client, _ = authed_api_client
        recipes_api = RecipesApi(client)

        recipe = recipes_api.create_recipe(
            CreateRecipeRequest(
                title="Manual Recipe",
                ingredients=[make_ingredient(item="test ingredient")],
                instructions="test instructions",
            )
        )

        with pytest.raises(ApiException) as exc_info:
            recipes_api.rescrape_photo(recipe.id)
        assert exc_info.value.status == 400

    def test_rescrape_photo_nonexistent_recipe(self, authed_api_client):
        client, _ = authed_api_client
        recipes_api = RecipesApi(client)

        with pytest.raises(ApiException) as exc_info:
            recipes_api.rescrape_photo("00000000-0000-0000-0000-000000000000")
        assert exc_info.value.status == 404

    def test_rescrape_photo_requires_auth(self, unauthed_api_client):
        recipes_api = RecipesApi(unauthed_api_client)
        with pytest.raises(ApiException) as exc_info:
            recipes_api.rescrape_photo("00000000-0000-0000-0000-000000000000")
        assert exc_info.value.status == 401


class TestRescrapeIsolation:
    """Test that rescrape respects user isolation."""

    def test_cannot_rescrape_other_users_recipe(
        self, authed_api_client, second_authed_api_client
    ):
        """Test that users cannot rescrape each other's recipes."""
        client1, _ = authed_api_client
        client2, _ = second_authed_api_client
        scrape_api1 = ScrapeApi(client1)
        recipes_api2 = RecipesApi(client2)

        # User 1 scrapes a recipe
        url = f"{FIXTURE_BASE_URL}/seriouseats/rice_pilaf.html"
        response = scrape_api1.create_scrape(CreateScrapeRequest(url=url))
        job = wait_for_job_completion(scrape_api1, response.id)

        assert job.status == "completed"
        recipe_id = job.recipe_id

        # User 2 tries to rescrape User 1's recipe
        with pytest.raises(ApiException) as exc_info:
            recipes_api2.rescrape(recipe_id)

        assert exc_info.value.status == 404
