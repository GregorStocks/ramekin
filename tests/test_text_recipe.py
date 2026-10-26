"""Text import runs as a background job, like every other import."""

import pytest

from conftest import wait_for_job_completion
from ramekin_client.api import ImportApi, NormalizeApi, RecipesApi, ScrapeApi
from ramekin_client.exceptions import ApiException


RECIPE_TEXT = """Text Pancakes
Serves 4. Prep: 10 minutes
1 cup all-purpose flour
8 oz butter
1 cup unknowable powder
Mix ingredients.

Cook in a pan.
Family notebook. Serve warm.
"""


def start(client, text=RECIPE_TEXT):
    return ImportApi(client).import_from_text({"text": text})


def import_text(client, text=RECIPE_TEXT):
    response = start(client, text)
    return wait_for_job_completion(ScrapeApi(client), str(response.job_id))


def step(job, name):
    return next(s for s in job.steps if s.name == name)


def test_text_import_saves_and_normalizes(authed_api_client):
    client, _ = authed_api_client
    response = start(client)
    assert response.status == "parsing"
    job = wait_for_job_completion(ScrapeApi(client), str(response.job_id))
    assert job.status == "completed", job.error
    assert job.url is None
    assert step(job, "extract_recipe").summary == 'extracted "Text Pancakes"'
    # Every default import op runs, house style included.
    for name in ("normalize_title", "generate_description", "house_style"):
        assert step(job, name).status == "completed", name
    assert step(job, "apply_normalize").status == "completed"

    recipe = RecipesApi(client).get_recipe(str(job.recipe_id))
    assert recipe.title == "Text Pancakes"
    assert recipe.servings == "4"
    assert recipe.prep_time == "10 minutes"
    assert recipe.source_name == "Family notebook"
    assert recipe.notes == "Serve warm."
    assert recipe.description == "A delicious test recipe."
    assert [i.item for i in recipe.ingredients] == [
        "all-purpose flour",
        "butter",
        "unknowable powder",
    ]
    assert [d.to_dict() for d in recipe.derived_measurements] == [
        {"ingredient_index": 0, "amount": "125", "unit": "g"},
        {"ingredient_index": 1, "amount": "227", "unit": "g"},
    ]


def test_text_import_is_its_recipes_source_for_reextract(authed_api_client):
    client, _ = authed_api_client
    job = import_text(client)
    assert job.status == "completed", job.error
    recipe_id = str(job.recipe_id)
    ops = {
        op.id: op
        for op in NormalizeApi(client).list_normalize_ops(recipe_id=recipe_id).ops
    }
    assert ops["reextract"].available
    assert not ops["refetch"].available


def test_model_warnings_show_on_the_job(authed_api_client):
    client, _ = authed_api_client
    job = import_text(client, "TEXT_MODEL_WARNING\n" + RECIPE_TEXT)
    assert job.status == "completed", job.error
    assert step(job, "extract_recipe").summary == (
        'extracted "Text Pancakes" · The source mentions a note that was not supplied.'
    )


def test_text_without_a_recipe_fails_without_saving(authed_api_client):
    client, _ = authed_api_client
    job = import_text(client, "TEXT_INCOMPLETE\n1 cup all-purpose flour")
    assert job.status == "failed"
    assert job.failed_at_step == "extract_recipe"
    assert job.error == (
        "No recipe found in the text (missing title, instructions). "
        "Check the missing fields."
    )
    assert job.recipe_id is None
    assert RecipesApi(client).list_recipes().recipes == []


def test_extraction_failure_fails_without_saving(authed_api_client):
    client, _ = authed_api_client
    job = import_text(client, "TEXT_EXTRACTION_FAILURE\n" + RECIPE_TEXT)
    assert job.status == "failed"
    assert job.failed_at_step == "extract_recipe"
    assert job.recipe_id is None
    assert RecipesApi(client).list_recipes().recipes == []


def test_failed_text_import_retries_from_its_text(authed_api_client):
    client, _ = authed_api_client
    scrape_api = ScrapeApi(client)
    job = import_text(client, "TEXT_EXTRACTION_FAILURE\n" + RECIPE_TEXT)
    assert job.can_retry

    retry = scrape_api.retry_scrape(job.id)
    assert retry.status == "parsing"
    # The fixture fails this text every time; the retry reran extraction.
    job = wait_for_job_completion(scrape_api, str(job.id))
    assert job.status == "failed"
    assert job.failed_at_step == "extract_recipe"
    assert job.retry_count == 1


def test_normalize_failure_keeps_the_saved_recipe(authed_api_client):
    client, _ = authed_api_client
    job = import_text(client, "TEXT_NORMALIZE_FAILURE\n" + RECIPE_TEXT)
    assert job.status == "failed"
    assert job.failed_at_step == "normalize_title"
    recipe = RecipesApi(client).get_recipe(str(job.recipe_id))
    assert recipe.title == "Force Text Normalize Failure"


@pytest.mark.parametrize("text", ["", "   ", "x" * 50_001])
def test_invalid_text_rejected(authed_api_client, text):
    client, _ = authed_api_client
    with pytest.raises(ApiException) as error:
        start(client, text)
    assert error.value.status == 400


def test_text_requires_authentication(unauthed_api_client):
    with pytest.raises(ApiException) as error:
        start(unauthed_api_client)
    assert error.value.status == 401


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
    recipe = RecipesApi(client).get_recipe(saved.id)
    assert len(recipe.ingredients[0].measurements) == 1
    assert recipe.derived_measurements[0].amount == "125"
