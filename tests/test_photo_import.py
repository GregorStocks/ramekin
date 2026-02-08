"""Tests for photo import functionality."""

import time

import pytest

from ramekin_client.api import ImportApi, PhotosApi, RecipesApi, ScrapeApi


def wait_for_job_completion(scrape_api, job_id, timeout=30.0):
    """Poll job status until completion or timeout."""
    start = time.time()
    while time.time() - start < timeout:
        job = scrape_api.get_scrape(job_id)
        if job.status in ("completed", "failed"):
            return job
        time.sleep(0.1)
    raise TimeoutError(f"Job {job_id} did not complete within {timeout}s")


class TestPhotoImport:
    """Test photo-based recipe import."""

    def test_import_from_photos_creates_recipe(self, authed_api_client, test_image):
        """Test that photos can be imported as a recipe."""
        client, user_id = authed_api_client
        photos_api = PhotosApi(client)
        import_api = ImportApi(client)
        scrape_api = ScrapeApi(client)
        recipes_api = RecipesApi(client)

        # Upload a test photo
        photo_response = photos_api.upload(file=("test.png", test_image))
        photo_id = photo_response.id

        # Start photo import
        response = import_api.import_from_photos(
            import_from_photos_request={"photo_ids": [photo_id]}
        )
        assert response.job_id is not None
        assert response.status == "pending"

        # Wait for completion
        job = wait_for_job_completion(scrape_api, response.job_id)
        assert job.status == "completed"
        assert job.recipe_id is not None

        # Verify recipe was created
        recipe = recipes_api.get_recipe(job.recipe_id)
        assert recipe.title == "Photo Imported Recipe"
        assert len(recipe.ingredients) > 0
        assert len(recipe.instructions) > 0
        assert photo_id in recipe.photo_ids

    def test_import_with_empty_photo_list_fails(self, authed_api_client):
        """Test that photo import requires at least one photo."""
        client, user_id = authed_api_client
        import_api = ImportApi(client)

        with pytest.raises(Exception):
            import_api.import_from_photos(import_from_photos_request={"photo_ids": []})

    def test_import_requires_auth(self, unauthed_api_client):
        """Test that photo import requires authentication."""
        import_api = ImportApi(unauthed_api_client)

        with pytest.raises(Exception):
            import_api.import_from_photos(
                import_from_photos_request={
                    "photo_ids": ["00000000-0000-0000-0000-000000000000"]
                }
            )
