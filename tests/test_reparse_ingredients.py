"""Re-parsing rebuilds each recipe's ingredients from the scrape that saved it."""

import json
import os

from conftest import connect_db, make_ingredient, wait_for_job_completion
from ramekin_client.api import ImportApi, RecipesApi, ScrapeApi
from ramekin_client.models import (
    CreateRecipeRequest,
    CreateScrapeRequest,
    UpdateRecipeRequest,
)

FIXTURE_BASE_URL = os.environ.get("FIXTURE_BASE_URL", "http://localhost:8888")

# What an older parser stored for the lines imported below.
STALE_INGREDIENTS = [
    {
        "item": "cloves garlic",
        "measurements": [{"amount": "2", "unit": "medium"}],
        "note": "minced",
        "section": None,
    },
    {
        "item": "Finely grated zest and juice of 1/2 lime",
        "measurements": [],
        "note": None,
        "section": None,
    },
]


def import_paprika(client, title, ingredients):
    job = ImportApi(client).import_recipe(
        {
            "raw_recipe": {
                "title": title,
                "ingredients": ingredients,
                "instructions": "Cook.",
            },
            "photo_ids": [],
            "extraction_method": "paprika",
        }
    )
    done = wait_for_job_completion(ScrapeApi(client), job.job_id)
    assert done.status == "completed"
    return done.recipe_id


def scrape(client, path):
    job = ScrapeApi(client).create_scrape(
        CreateScrapeRequest(url=f"{FIXTURE_BASE_URL}/{path}")
    )
    done = wait_for_job_completion(ScrapeApi(client), job.id)
    assert done.status == "completed"
    return done.recipe_id


def store_stale_version(database_url, recipe_id, ingredients):
    """Rewrite the current version as a recipe stored before a parser fix
    looks: older ingredients, saved by a data migration rather than by hand."""
    conn = connect_db(database_url)
    try:
        conn.execute(
            "UPDATE recipe_versions SET ingredients = ?, version_source = 'migration' "
            "WHERE id = (SELECT current_version_id FROM recipes WHERE id = ?)",
            (json.dumps(ingredients), str(recipe_id)),
        )
    finally:
        conn.close()


def test_reparse_rebuilds_stale_ingredients_from_the_import(
    authed_api_client, database_url
):
    client, _ = authed_api_client
    api = RecipesApi(client)
    recipe_id = import_paprika(
        client,
        "Garlic Lime Rice",
        "2 medium cloves garlic, minced\nFinely grated zest and juice of 1/2 lime",
    )
    fresh = api.get_recipe(recipe_id).ingredients
    store_stale_version(database_url, recipe_id, STALE_INGREDIENTS)
    assert api.get_recipe(recipe_id).ingredients[0].item == "cloves garlic"

    result = api.reparse_all_ingredients()
    assert (result.recipes_checked, result.recipes_updated) == (1, 1)

    after = api.get_recipe(recipe_id)
    assert after.version_source == "reparse"
    assert after.ingredients == fresh
    assert [i.item for i in after.ingredients] == ["garlic", "lime zest", "lime juice"]
    # The stale version is kept, not replaced.
    sources = [v.version_source for v in api.list_versions(recipe_id).versions]
    assert sources[:2] == ["reparse", "migration"]

    # Nothing left to change on a second run.
    assert api.reparse_all_ingredients().recipes_updated == 0


def test_reparse_reextracts_a_scraped_page(authed_api_client, database_url):
    client, _ = authed_api_client
    api = RecipesApi(client)
    recipe_id = scrape(client, "seriouseats/rice_pilaf.html")
    fresh = api.get_recipe(recipe_id).ingredients
    store_stale_version(database_url, recipe_id, STALE_INGREDIENTS)

    result = api.reparse_all_ingredients()
    assert result.recipes_updated == 1
    assert api.get_recipe(recipe_id).ingredients == fresh


def test_reparse_skips_hand_edits_and_recipes_without_a_scrape(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    edited = import_paprika(client, "Edited", "2 medium cloves garlic")
    current = api.get_recipe(edited)
    api.update_recipe(
        edited,
        UpdateRecipeRequest(
            expected_version_id=current.version_id,
            title=current.title,
            instructions=current.instructions,
            ingredients=[make_ingredient("my garlic", "2", "clove")],
            tags=[],
        ),
    )
    typed = api.create_recipe(
        CreateRecipeRequest(
            title="Typed",
            instructions="Cook.",
            ingredients=[make_ingredient("cloves garlic", "2", "medium")],
            tags=[],
        )
    ).id

    result = api.reparse_all_ingredients()
    assert result.recipes_checked == 2
    assert result.recipes_updated == 0
    assert (result.skipped_edited, result.skipped_no_source) == (1, 1)
    assert api.get_recipe(edited).ingredients[0].item == "my garlic"
    assert api.get_recipe(typed).version_source == "user"


def test_reparse_only_touches_the_callers_recipes(
    authed_api_client, second_authed_api_client, database_url
):
    client, _ = authed_api_client
    other_client, _ = second_authed_api_client
    theirs = import_paprika(other_client, "Theirs", "2 medium cloves garlic")
    store_stale_version(database_url, theirs, STALE_INGREDIENTS[:1])

    assert RecipesApi(client).reparse_all_ingredients().recipes_checked == 0
    assert RecipesApi(other_client).get_recipe(theirs).version_source == "migration"
