"""Seeding waits for saved recipes and reports asynchronous enrichment failures."""

import gzip
import json
import os
import re
import subprocess
import uuid
import zipfile

import pytest

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, RecipesApi, ScrapeApi


@pytest.mark.parametrize("auth_failure", [False, True])
def test_seed_reports_finished_jobs_and_preserves_recipes(
    server_url, tmp_path, auth_failure
):
    username = f"seed-{uuid.uuid4()}"
    title = "SeedAuthFailureFixture" if auth_failure else "Seed Soup"
    archive = tmp_path / "seed.paprikarecipes"
    with zipfile.ZipFile(archive, "w") as output:
        output.writestr(
            "soup.paprikarecipe",
            gzip.compress(
                json.dumps(
                    {
                        "name": title,
                        "ingredients": "1 cup water",
                        "directions": "Boil the water.",
                    }
                ).encode()
            ),
        )
    tags = tmp_path / "tags.json"
    tags.write_text(json.dumps({"tags": ["Soup"]}))
    result = subprocess.run(
        [
            os.environ["CLI_PATH"],
            "seed",
            "--server-url",
            server_url,
            "--username",
            username,
            "--password",
            "seed-password",
            "--tags-file",
            str(tags),
            str(archive),
        ],
        capture_output=True,
        text=True,
        timeout=60,
        env={**os.environ, "RUST_LOG": "info"},
    )
    assert result.returncode == 0, result.stderr
    assert "Recipes saved: 1" in result.stderr
    assert f"Recipes with enrichment failures: {int(auth_failure)}" in result.stderr
    assert "test-api-key" not in result.stderr

    config = Configuration(host=server_url)
    with ApiClient(config) as client:
        login = AuthApi(client).login(
            {"username": username, "password": "seed-password"}
        )
        config.access_token = login.token
        recipes = RecipesApi(client).list_recipes().recipes
        assert len(recipes) == 1
        recipe = RecipesApi(client).get_recipe(recipes[0].id)
        assert recipe.instructions == "Boil the water."

        # No polling here: seed must have waited for the terminal job state.
        job_id = re.search(r"job_id: ([0-9a-f-]{36})", result.stderr).group(1)
        job = ScrapeApi(client).get_scrape(job_id)
        assert job.status == ("failed" if auth_failure else "completed")
        if auth_failure:
            assert "Recipe saved, but enrichment failed" in result.stderr
            failed = next(step for step in job.steps if step.status == "failed")
            assert "Missing Authentication header" in failed.error
            assert recipe.title == title
