"""Text creation must use import processing, without saving before review."""

import pytest

from conftest import wait_for_job_completion
from ramekin_client.api import ImportApi, RecipesApi, ScrapeApi
from ramekin_client.exceptions import ApiException


RECIPE_TEXT = """Text Pancakes
Serves 4. Prep: 10 minutes
1 cup all-purpose flour
8 oz butter
1 cup mystery powder
Mix ingredients.

Cook in a pan.
Family notebook. Serve warm.
"""


def prepare(client, text=RECIPE_TEXT):
    return ImportApi(client).prepare_text_recipe({"text": text})


def test_text_draft_review_save_and_import_parity(authed_api_client):
    client, _ = authed_api_client
    recipes = RecipesApi(client)
    draft = prepare(client)
    assert draft.content.title == "Text Pancakes"
    assert draft.content.servings == "4"
    assert draft.content.prep_time == "10 minutes"
    assert draft.content.source_name == "Family notebook"
    assert draft.content.notes == "Serve warm."
    assert draft.warnings == ["Weight estimate unavailable for mystery powder."]
    ingredients = draft.content.ingredients
    assert ingredients[0].measurements[-1].amount == "125"
    assert ingredients[0].measurements[-1].unit == "g"
    assert ingredients[1].measurements[-1].amount == "227"
    assert len(ingredients[2].measurements) == 1
    assert recipes.list_recipes().recipes == []

    saved = recipes.create_recipe(
        {
            **draft.content.to_dict(),
            "raw_ingredients": draft.raw_ingredients,
        }
    )
    recipe = recipes.get_recipe(saved.id)
    assert recipe.ingredients == ingredients

    imported = ImportApi(client).import_recipe(
        {
            "raw_recipe": {
                "title": "Text Pancakes",
                "ingredients": draft.raw_ingredients,
                "instructions": draft.content.instructions,
            },
            "photo_ids": [],
            "extraction_method": "paprika",
        }
    )
    job = wait_for_job_completion(ScrapeApi(client), imported.job_id)
    assert job.status == "completed"
    assert recipes.get_recipe(job.recipe_id).ingredients == ingredients


def test_reviewed_ingredient_changes_recompute_weights(authed_api_client):
    client, _ = authed_api_client
    draft = prepare(client)
    saved = RecipesApi(client).create_recipe(
        {
            **draft.content.to_dict(),
            "title": "Corrected Pancakes",
            "raw_ingredients": "2 cups all-purpose flour\n1 cup mystery powder",
        }
    )
    recipe = RecipesApi(client).get_recipe(saved.id)
    assert recipe.title == "Corrected Pancakes"
    assert recipe.ingredients[0].measurements[-1].amount == "250"
    assert len(recipe.ingredients[1].measurements) == 1


def test_structured_creation_gets_weight_estimates(authed_api_client):
    client, _ = authed_api_client
    saved = RecipesApi(client).create_recipe(
        {
            "title": "Manual",
            "instructions": "Mix.",
            "ingredients": [
                {
                    "item": "all-purpose flour",
                    "measurements": [
                        {"amount": "1", "unit": "cup"},
                    ],
                }
            ],
        }
    )
    assert (
        RecipesApi(client).get_recipe(saved.id).ingredients[0].measurements[-1].amount
        == "125"
    )


def test_incomplete_text_exposes_missing_fields(authed_api_client):
    client, _ = authed_api_client
    draft = prepare(client, "TEXT_INCOMPLETE\n1 cup all-purpose flour")
    assert draft.content.title == ""
    assert draft.content.instructions == ""
    assert "Missing title. Add it before saving." in draft.warnings
    assert "Missing instructions. Add it before saving." in draft.warnings
    with pytest.raises(ApiException) as error:
        RecipesApi(client).create_recipe(
            {**draft.content.to_dict(), "raw_ingredients": draft.raw_ingredients}
        )
    assert error.value.status == 400


@pytest.mark.parametrize("text", ["", "   ", "x" * 50_001])
def test_invalid_text_rejected(authed_api_client, text):
    client, _ = authed_api_client
    with pytest.raises(ApiException) as error:
        prepare(client, text)
    assert error.value.status == 400


@pytest.mark.parametrize(
    "marker", ["TEXT_EXTRACTION_FAILURE", "TEXT_ENRICHMENT_FAILURE"]
)
def test_pipeline_failure_does_not_save(authed_api_client, marker):
    client, _ = authed_api_client
    with pytest.raises(ApiException) as error:
        prepare(client, marker + "\nFailure regression\n" + RECIPE_TEXT)
    assert error.value.status == 503
    assert RecipesApi(client).list_recipes().recipes == []


def test_text_requires_authentication(unauthed_api_client):
    with pytest.raises(ApiException) as error:
        prepare(unauthed_api_client)
    assert error.value.status == 401


@pytest.mark.parametrize("lines", ["   ", "Ingredients:"])
def test_raw_ingredients_must_contain_ingredients(authed_api_client, lines):
    client, _ = authed_api_client
    with pytest.raises(ApiException) as error:
        RecipesApi(client).create_recipe(
            {
                "title": "Incomplete recipe",
                "instructions": "Mix.",
                "ingredients": [],
                "raw_ingredients": lines,
            }
        )
    assert error.value.status == 400
