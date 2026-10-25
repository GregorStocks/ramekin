"""Edit with AI: a job that edits a recipe with tools and proposes a version.

The mock AI prefixes the title with "[Modified] " ("[Modified with Photo] "
when photos came along), looks an ingredient up, then finishes with a
summary.
"""

import pytest

from conftest import make_ingredient, wait_for_job_completion
from ramekin_client.api import (
    EditRecipeApi,
    NormalizeApi,
    PhotosApi,
    RecipesApi,
    ScrapeApi,
)
from ramekin_client.exceptions import ApiException
from ramekin_client.models import (
    CreateRecipeRequest,
    EditRecipeRequest,
    UpdateRecipeRequest,
)


def create_recipe(client, title="Chicken Stir Fry") -> str:
    return (
        RecipesApi(client)
        .create_recipe(
            CreateRecipeRequest(
                title=title,
                instructions="Cook chicken with vegetables in a wok.",
                ingredients=[
                    make_ingredient("chicken breast", "1", "lb"),
                    make_ingredient("soy sauce", "2", "tbsp"),
                ],
                tags=["dinner"],
            )
        )
        .id
    )


def run_edit(client, recipe_id, instruction="make this vegan", photo_ids=None):
    """Start an edit, wait for it, and return its job id and proposal."""
    job = EditRecipeApi(client).edit_recipe_with_ai(
        recipe_id,
        EditRecipeRequest(instruction=instruction, photo_ids=photo_ids or []),
    )
    finished = wait_for_job_completion(ScrapeApi(client), str(job.job_id))
    assert finished.status == "completed", finished.error
    proposal = NormalizeApi(client).get_normalize_proposal(recipe_id, str(job.job_id))
    return str(job.job_id), proposal


def test_edit_requires_auth(unauthed_api_client):
    with pytest.raises(ApiException) as exc_info:
        EditRecipeApi(unauthed_api_client).edit_recipe_with_ai(
            "00000000-0000-0000-0000-000000000000",
            EditRecipeRequest(instruction="make it vegan"),
        )
    assert exc_info.value.status == 401


def test_edit_proposes_then_accept_saves_an_ai_edit_version(authed_api_client):
    client, _user_id = authed_api_client
    recipe_id = create_recipe(client)

    job_id, proposal = run_edit(client, recipe_id)

    assert proposal.proposed.content.title == "[Modified] Chicken Stir Fry"
    assert proposal.summary == "Marked the title as modified."
    assert proposal.base.content.title == "Chicken Stir Fry"
    # Untouched fields come through unchanged.
    assert proposal.proposed.content.instructions == proposal.base.content.instructions
    assert proposal.proposed.content.ingredients == proposal.base.content.ingredients
    # Nothing is saved until the proposal is accepted.
    recipes = RecipesApi(client)
    assert recipes.get_recipe(recipe_id).title == "Chicken Stir Fry"

    NormalizeApi(client).accept_normalize_proposal(recipe_id, job_id)

    recipe = recipes.get_recipe(recipe_id)
    assert recipe.title == "[Modified] Chicken Stir Fry"
    assert recipe.version_source == "ai_edit"
    assert recipe.tags == ["dinner"]


def test_edit_job_shows_its_steps(authed_api_client):
    client, _user_id = authed_api_client
    recipe_id = create_recipe(client)
    job = EditRecipeApi(client).edit_recipe_with_ai(
        recipe_id, EditRecipeRequest(instruction="make it spicier")
    )
    finished = wait_for_job_completion(ScrapeApi(client), str(job.job_id))
    assert [step.name for step in finished.steps] == [
        "load_recipe",
        "edit_with_ai",
        "propose_normalize",
    ]


def test_edit_uses_recipe_photos(authed_api_client, test_image):
    client, _user_id = authed_api_client
    recipe_id = create_recipe(client)
    photo_id = PhotosApi(client).upload(file=("test.png", test_image)).id

    _job_id, proposal = run_edit(client, recipe_id, photo_ids=[photo_id, photo_id])

    assert proposal.proposed.content.title == "[Modified with Photo] Chicken Stir Fry"


def test_edit_rejects_other_users_photos(
    authed_api_client, second_authed_api_client, test_image
):
    client, _user_id = authed_api_client
    other_client, _other_user_id = second_authed_api_client
    other_photo_id = PhotosApi(other_client).upload(file=("test.png", test_image)).id
    recipe_id = create_recipe(client)

    with pytest.raises(ApiException) as exc_info:
        EditRecipeApi(client).edit_recipe_with_ai(
            recipe_id,
            EditRecipeRequest(instruction="make it vegan", photo_ids=[other_photo_id]),
        )
    assert exc_info.value.status == 400


def test_edit_rejects_an_empty_instruction(authed_api_client):
    client, _user_id = authed_api_client
    recipe_id = create_recipe(client)

    with pytest.raises(ApiException) as exc_info:
        EditRecipeApi(client).edit_recipe_with_ai(
            recipe_id, EditRecipeRequest(instruction="   ")
        )
    assert exc_info.value.status == 400


def test_edit_of_another_users_recipe_is_not_found(
    authed_api_client, second_authed_api_client
):
    client, _user_id = authed_api_client
    other_client, _other_user_id = second_authed_api_client
    recipe_id = create_recipe(client)

    with pytest.raises(ApiException) as exc_info:
        EditRecipeApi(other_client).edit_recipe_with_ai(
            recipe_id, EditRecipeRequest(instruction="make it vegan")
        )
    assert exc_info.value.status == 404


def test_accepting_after_a_hand_edit_conflicts(authed_api_client):
    client, _user_id = authed_api_client
    recipe_id = create_recipe(client)
    job_id, _proposal = run_edit(client, recipe_id)
    recipes = RecipesApi(client)
    current = recipes.get_recipe(recipe_id)
    recipes.update_recipe(
        recipe_id,
        UpdateRecipeRequest(expected_version_id=current.version_id, title="Mine"),
    )

    with pytest.raises(ApiException) as exc_info:
        NormalizeApi(client).accept_normalize_proposal(recipe_id, job_id)
    assert exc_info.value.status == 409
    assert recipes.get_recipe(recipe_id).title == "Mine"


def test_edit_searches_the_users_other_recipes_by_ingredient(authed_api_client):
    client, _user_id = authed_api_client
    RecipesApi(client).create_recipe(
        CreateRecipeRequest(
            title="Hummus",
            instructions="Blend.",
            ingredients=[make_ingredient("tahini", "1/4", "cup")],
        )
    )
    recipe_id = create_recipe(client, title="Falafel Bowl")

    _job_id, proposal = run_edit(
        client, recipe_id, instruction="make the sauce like my other one"
    )

    assert proposal.summary == "Found: Title: Hummus"
