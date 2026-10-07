"""End-to-end tests for the post-save ("enriching") phase of scrape jobs.

Once save_recipe runs, the job reports status "enriching" with the recipe id
so clients can show the recipe while the AI steps finish, and those steps run
concurrently instead of one at a time.
"""

import json
import os
import time
import uuid

import requests

from conftest import make_ingredient, wait_for_job_completion
from ramekin_client.api import RecipesApi, ScrapeApi
from ramekin_client.models import CreateRecipeRequest, UpdateRecipeRequest


def unique(label: str) -> str:
    # Letters only, so the token survives title normalization intact.
    token = uuid.uuid4().hex[:8].translate(str.maketrans("0123456789", "ghijklmnop"))
    return f"zq{token} {label}"


def mock_get(path: str, **params) -> None:
    port = os.environ["MOCK_OPENROUTER_PORT"]
    requests.get(
        f"http://localhost:{port}{path}", params=params, timeout=10
    ).raise_for_status()


def hold_description(marker: str, hold: bool) -> None:
    mock_get("/test/description-hold", marker=marker, hold=str(hold).lower())


def capture(server_url: str, token: str, title: str, ingredient: str) -> str:
    recipe = {
        "@context": "https://schema.org",
        "@type": "Recipe",
        "name": title,
        "recipeIngredient": [f"1 cup {ingredient}", "2 cups water"],
        "recipeInstructions": [{"@type": "HowToStep", "text": "Stir and serve."}],
    }
    html = (
        "<html><head><script type='application/ld+json'>"
        f"{json.dumps(recipe)}</script></head><body></body></html>"
    )
    response = requests.post(
        f"{server_url}/api/scrape/capture",
        json={
            "html": html,
            "source_url": f"https://www.seriouseats.com/{uuid.uuid4().hex}",
        },
        headers={"Authorization": f"Bearer {token}"},
        timeout=10,
    )
    response.raise_for_status()
    return response.json()["id"]


def wait_for(predicate, timeout: float = 15.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(0.1)
    raise TimeoutError("condition not met")


def step_status(job, name: str) -> str:
    return next(s.status for s in job.steps if s.name == name)


def give_user_auto_tag(recipes_api: RecipesApi) -> None:
    """Auto-tagging only applies tags the user already has; the mock suggests
    'test-auto-tag'."""
    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Setup Recipe for Tags",
            ingredients=[make_ingredient(item="water")],
            instructions="test instructions",
            tags=["test-auto-tag"],
        )
    )


def test_recipe_is_viewable_while_enrichment_runs_concurrently(
    authed_api_client, server_url
):
    client, _ = authed_api_client
    scrape_api = ScrapeApi(client)
    recipes_api = RecipesApi(client)
    give_user_auto_tag(recipes_api)
    marker = unique("soup")

    # Hold the description call, the slowest AI step in production.
    hold_description(marker, True)
    try:
        job_id = capture(
            server_url, client.configuration.access_token, f"Viewable {marker}", "salt"
        )
        # Steps that don't depend on the description finish while it is
        # still held: they no longer run one after another.
        job = wait_for(
            lambda: (
                (j := scrape_api.get_scrape(job_id))
                and all(
                    step_status(j, s) == "completed"
                    for s in (
                        "resolve_ingredient_names",
                        "enrich_normalize_title",
                        "apply_normalized_title",
                        "enrich_auto_tag",
                    )
                )
                and j
            )
        )
        assert job.status == "enriching"
        assert step_status(job, "enrich_generate_description") == "running"
        assert step_status(job, "apply_auto_tags") == "pending"
        assert job.recipe_id is not None
        recipe = recipes_api.get_recipe(id=str(job.recipe_id))
        assert recipe.title == f"Viewable {marker}"
    finally:
        hold_description(marker, False)

    job = wait_for_job_completion(scrape_api, job_id)
    assert job.status == "completed", job.error
    recipe = recipes_api.get_recipe(id=str(job.recipe_id))
    assert recipe.description == "A delicious test recipe."
    assert "test-auto-tag" in recipe.tags


def test_edit_during_enrichment_wins_over_ai(authed_api_client, server_url):
    client, _ = authed_api_client
    scrape_api = ScrapeApi(client)
    recipes_api = RecipesApi(client)
    give_user_auto_tag(recipes_api)
    marker = unique("stew")

    hold_description(marker, True)
    try:
        job_id = capture(
            server_url, client.configuration.access_token, f"Edited {marker}", "salt"
        )
        job = wait_for(
            lambda: (
                (j := scrape_api.get_scrape(job_id))
                and j.status == "enriching"
                and step_status(j, "apply_normalized_title") == "completed"
                and step_status(j, "enrich_auto_tag") == "completed"
                and j
            )
        )
        recipe_id = str(job.recipe_id)
        current = recipes_api.get_recipe(id=recipe_id)
        recipes_api.update_recipe(
            id=recipe_id,
            update_recipe_request=UpdateRecipeRequest(
                expected_version_id=current.version_id,
                title="My own title",
            ),
        )
    finally:
        hold_description(marker, False)

    job = wait_for_job_completion(scrape_api, job_id)
    # The edit made the AI writes stale; they are skipped, not failures.
    assert job.status == "completed", job.error
    for name in ("apply_generated_description", "apply_auto_tags"):
        step = next(s for s in job.steps if s.name == name)
        assert step.status == "completed"
        assert step.summary.startswith("skipped:"), step.summary
    recipe = recipes_api.get_recipe(id=recipe_id)
    assert recipe.title == "My own title"
    assert recipe.description != "A delicious test recipe."
    assert "test-auto-tag" not in recipe.tags
