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


def write_archive(path, titles):
    with zipfile.ZipFile(path, "w") as output:
        for title in titles:
            output.writestr(
                f"{title}.paprikarecipe",
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


def run_cli(command, server_url, username, archive):
    result = subprocess.run(
        [
            os.environ["CLI_PATH"],
            command,
            "--server-url",
            server_url,
            "--username",
            username,
            "--password",
            "seed-password",
            str(archive),
        ],
        capture_output=True,
        text=True,
        timeout=60,
        env={**os.environ, "RUST_LOG": "info"},
    )
    assert result.returncode == 0, result.stderr
    return result.stderr


def recipe_titles(server_url, username):
    config = Configuration(host=server_url)
    with ApiClient(config) as client:
        login = AuthApi(client).login(
            {"username": username, "password": "seed-password"}
        )
        config.access_token = login.token
        return sorted(r.title for r in RecipesApi(client).list_recipes().recipes)


def test_seed_resumes_incomplete_import_without_duplicates(server_url, tmp_path):
    username = f"seed-{uuid.uuid4()}"
    # A seed interrupted after submitting only its first recipe.
    first = tmp_path / "first.paprikarecipes"
    write_archive(first, ["Seed Soup"])
    run_cli("seed", server_url, username, first)

    full = tmp_path / "full.paprikarecipes"
    write_archive(full, ["Seed Soup", "Seed Bread"])
    stderr = run_cli("seed", server_url, username, full)
    assert "1 of 2 seed recipe(s) already submitted" in stderr
    assert "Submitted: Seed Bread" in stderr
    assert "Submitted: Seed Soup" not in stderr
    assert "Recipes saved: 2" in stderr
    assert recipe_titles(server_url, username) == ["Seed Bread", "Seed Soup"]

    stderr = run_cli("seed", server_url, username, full)
    assert "2 of 2 seed recipe(s) already submitted" in stderr
    assert "Submitted:" not in stderr
    assert recipe_titles(server_url, username) == ["Seed Bread", "Seed Soup"]


def test_seed_skips_user_seeded_without_import_keys(server_url, tmp_path):
    username = f"seed-{uuid.uuid4()}"
    archive = tmp_path / "seed.paprikarecipes"
    write_archive(archive, ["Seed Soup"])
    with ApiClient(Configuration(host=server_url)) as client:
        AuthApi(client).signup({"username": username, "password": "seed-password"})
    # `import` sends no idempotency keys, like seeds made before them.
    run_cli("import", server_url, username, archive)

    stderr = run_cli("seed", server_url, username, archive)
    assert "seed without import keys, skipping seed" in stderr
    assert recipe_titles(server_url, username) == ["Seed Soup"]


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
