"""End-to-end tests for normalize ops: listing them, running any subset on a
recipe as a job that proposes a version, and accepting the proposal."""

import os
from pathlib import Path
from uuid import uuid4

import pytest

from conftest import connect_db, make_ingredient, wait_for_job_completion
from ramekin_client.api import (
    ImportApi,
    NormalizeApi,
    PhotosApi,
    RecipesApi,
    ScrapeApi,
    TagsApi,
)
from ramekin_client.exceptions import ApiException
from ramekin_client.models import (
    BulkNormalizeRequest,
    CreateRecipeRequest,
    CreateScrapeRequest,
    NormalizeRequest,
    RenameTagRequest,
    UpdateRecipeRequest,
)

FIXTURE_BASE_URL = os.environ.get("FIXTURE_BASE_URL", "http://localhost:8888")

RECIPE_HTML = """<!DOCTYPE html>
<html><head><script type="application/ld+json">
{{
  "@context": "https://schema.org",
  "@type": "Recipe",
  "name": "{title}",
  "recipeIngredient": ["1 cup flour", "2 eggs"],
  "recipeInstructions": "Mix and bake."
}}
</script></head><body></body></html>
"""


def write_fixture(title: str) -> tuple[Path, str]:
    """Serve a recipe page from the fixture server; returns its path and URL."""
    path = Path(__file__).parent / "scrape_fixtures" / f"normalize-{uuid4().hex}.html"
    path.write_text(RECIPE_HTML.format(title=title))
    return path, f"{FIXTURE_BASE_URL}/{path.name}"


def scrape(client, url: str) -> str:
    scrape_api = ScrapeApi(client)
    job = wait_for_job_completion(
        scrape_api, scrape_api.create_scrape(CreateScrapeRequest(url=url)).id
    )
    assert job.status == "completed", job.error
    return str(job.recipe_id)


def create_typed_recipe(client, title: str, tags=None) -> str:
    return str(
        RecipesApi(client)
        .create_recipe(
            CreateRecipeRequest(
                title=title,
                instructions="Cook it.",
                ingredients=[make_ingredient(item="shrimp", amount="1", unit="lb")],
                tags=tags or [],
            )
        )
        .id
    )


def run(client, recipe_id: str, ops: list[str]):
    """Run a normalize job to completion; returns its proposal."""
    normalize_api = NormalizeApi(client)
    job = normalize_api.normalize_recipe(recipe_id, NormalizeRequest(ops=ops))
    finished = wait_for_job_completion(ScrapeApi(client), str(job.job_id))
    assert finished.status == "completed", finished.error
    assert str(finished.recipe_id) == recipe_id
    return job.job_id, normalize_api.get_normalize_proposal(recipe_id, str(job.job_id))


def ops_by_id(response):
    return {op.id: op for op in response.ops}


def test_normalize_requires_auth(unauthed_api_client):
    with pytest.raises(ApiException) as error:
        NormalizeApi(unauthed_api_client).normalize_recipe(
            "00000000-0000-0000-0000-000000000000",
            NormalizeRequest(ops=["normalize_title"]),
        )
    assert error.value.status == 401


def test_list_ops_describes_the_registry(authed_api_client):
    client, _ = authed_api_client
    response = NormalizeApi(client).list_normalize_ops()
    assert [op.id for op in response.ops] == [
        "refetch",
        "reextract",
        "normalize_title",
        "generate_description",
        "auto_tag",
        "house_style",
    ]
    ops = ops_by_id(response)
    assert all(op.available for op in response.ops)
    assert ops["refetch"].implies == ["reextract"]
    assert ops["refetch"].group == "source"
    assert ops["generate_description"].runs_after == ["normalize_title"]
    assert ops["auto_tag"].writes == ["tags"]
    assert ops["house_style"].writes == ["ingredients", "instructions"]
    assert [op.id for op in response.ops if op.default_on_import] == [
        "normalize_title",
        "generate_description",
        "auto_tag",
        "house_style",
    ]


def test_house_style_proposes_rewritten_steps(authed_api_client):
    client, _ = authed_api_client
    # The mock rewrites only recipes titled for this test: it adds a prep step.
    recipe_id = create_typed_recipe(client, "House Style Test Soup")

    _, proposal = run(client, recipe_id, ["house_style"])

    assert proposal.ops == ["house_style"]
    proposed = proposal.proposed.content
    assert proposed.instructions == "Chop the onion.\nCook it."
    assert [i.item for i in proposed.ingredients] == ["shrimp"]
    assert proposed.ingredients[0].measurements[0].amount == "1"
    assert proposed.title == "House Style Test Soup"


def test_house_style_leaves_a_styled_recipe_alone(authed_api_client):
    client, _ = authed_api_client
    recipe_id = create_typed_recipe(client, "Plain Shrimp")

    _, proposal = run(client, recipe_id, ["house_style"])

    assert proposal.proposed.content.instructions == "Cook it."


def test_typed_recipe_has_no_source_ops(authed_api_client):
    client, _ = authed_api_client
    recipe_id = create_typed_recipe(client, "Typed Shrimp")
    ops = ops_by_id(NormalizeApi(client).list_normalize_ops(recipe_id=recipe_id))
    assert not ops["refetch"].available
    assert ops["refetch"].unavailable_reason == "Recipe has no source URL"
    assert not ops["reextract"].available
    assert ops["normalize_title"].available

    with pytest.raises(ApiException) as error:
        NormalizeApi(client).normalize_recipe(
            recipe_id, NormalizeRequest(ops=["reextract"])
        )
    assert error.value.status == 400


@pytest.mark.parametrize("ops", [[], ["enrich"]])
def test_unknown_or_no_ops_are_rejected(authed_api_client, ops):
    client, _ = authed_api_client
    recipe_id = create_typed_recipe(client, "Rejected Shrimp")
    with pytest.raises(ApiException) as error:
        NormalizeApi(client).normalize_recipe(recipe_id, NormalizeRequest(ops=ops))
    assert error.value.status == 400


def test_other_users_recipe_is_not_found(authed_api_client, second_authed_api_client):
    client, _ = authed_api_client
    other_client, _ = second_authed_api_client
    recipe_id = create_typed_recipe(client, "Private Shrimp")
    with pytest.raises(ApiException) as error:
        NormalizeApi(other_client).normalize_recipe(
            recipe_id, NormalizeRequest(ops=["normalize_title"])
        )
    assert error.value.status == 404


def test_run_proposes_then_accept_saves_one_version(authed_api_client):
    client, _ = authed_api_client
    recipes_api = RecipesApi(client)
    # Auto-tagging only applies tags the user already has; the mock suggests
    # 'test-auto-tag'.
    create_typed_recipe(client, "Tag Seed", tags=["test-auto-tag"])
    # The mock normalizer collapses whitespace runs, so this title changes.
    recipe_id = create_typed_recipe(client, "Garlic  Shrimp", tags=["dinner"])
    before = recipes_api.get_recipe(recipe_id)

    job_id, proposal = run(
        client, recipe_id, ["auto_tag", "generate_description", "normalize_title"]
    )
    assert proposal.status == "completed"
    assert proposal.ops == ["normalize_title", "generate_description", "auto_tag"]
    assert str(proposal.base_version_id) == str(before.version_id)
    assert proposal.base.content.title == "Garlic  Shrimp"
    proposed = proposal.proposed.content
    assert proposed.title == "Garlic Shrimp"
    assert proposed.description == "A delicious test recipe."
    assert sorted(proposed.tags) == ["dinner", "test-auto-tag"]
    # Proposing saves nothing.
    assert recipes_api.get_recipe(recipe_id).version_id == before.version_id

    accepted = NormalizeApi(client).accept_normalize_proposal(recipe_id, str(job_id))
    recipe = recipes_api.get_recipe(recipe_id)
    assert recipe.version_id == accepted.version_id
    assert recipe.version_source == "normalize"
    assert recipe.normalize_ops == [
        "normalize_title",
        "generate_description",
        "auto_tag",
    ]
    assert recipe.title == "Garlic Shrimp"
    assert sorted(recipe.tags) == ["dinner", "test-auto-tag"]
    versions = recipes_api.list_versions(recipe_id).versions
    assert [v.version_source for v in versions] == ["normalize", "user"]
    assert versions[0].normalize_ops == recipe.normalize_ops

    # The proposal was for the version it replaced: accepting again conflicts.
    with pytest.raises(ApiException) as error:
        NormalizeApi(client).accept_normalize_proposal(recipe_id, str(job_id))
    assert error.value.status == 409


def test_accept_after_an_edit_conflicts(authed_api_client):
    client, _ = authed_api_client
    recipes_api = RecipesApi(client)
    recipe_id = create_typed_recipe(client, "Lemon  Shrimp")
    job_id, _ = run(client, recipe_id, ["normalize_title"])

    current = recipes_api.get_recipe(recipe_id)
    recipes_api.update_recipe(
        recipe_id,
        UpdateRecipeRequest(expected_version_id=current.version_id, title="Mine"),
    )
    with pytest.raises(ApiException) as error:
        NormalizeApi(client).accept_normalize_proposal(recipe_id, str(job_id))
    assert error.value.status == 409
    assert recipes_api.get_recipe(recipe_id).title == "Mine"


def test_proposal_of_another_recipe_is_not_found(authed_api_client):
    client, _ = authed_api_client
    recipe_id = create_typed_recipe(client, "Basil  Shrimp")
    other_id = create_typed_recipe(client, "Other Shrimp")
    job_id, _ = run(client, recipe_id, ["normalize_title"])
    with pytest.raises(ApiException) as error:
        NormalizeApi(client).get_normalize_proposal(other_id, str(job_id))
    assert error.value.status == 404


def test_reextract_rebuilds_from_the_saved_page(authed_api_client):
    client, _ = authed_api_client
    recipes_api = RecipesApi(client)
    path, url = write_fixture("Saved Page Cake")
    try:
        recipe_id = scrape(client, url)
    finally:
        # The page is gone: re-extracting must not fetch it again.
        path.unlink(missing_ok=True)

    current = recipes_api.get_recipe(recipe_id)
    recipes_api.update_recipe(
        recipe_id,
        UpdateRecipeRequest(
            expected_version_id=current.version_id,
            title="Hand Edited",
            ingredients=[make_ingredient(item="sugar")],
        ),
    )

    job_id, proposal = run(client, recipe_id, ["reextract"])
    proposed = proposal.proposed.content
    assert proposal.ops == ["reextract"]
    assert proposed.title == "Saved Page Cake"
    assert [i.item for i in proposed.ingredients] == ["flour", "eggs"]
    assert proposed.source_url == url

    NormalizeApi(client).accept_normalize_proposal(recipe_id, str(job_id))
    recipe = recipes_api.get_recipe(recipe_id)
    assert recipe.title == "Saved Page Cake"
    assert recipe.normalize_ops == ["reextract"]


def test_refetch_reads_the_page_again(authed_api_client):
    client, _ = authed_api_client
    path, url = write_fixture("First Title Bread")
    try:
        recipe_id = scrape(client, url)
        path.write_text(RECIPE_HTML.format(title="Second Title Bread"))
        _, proposal = run(client, recipe_id, ["refetch"])
    finally:
        path.unlink(missing_ok=True)
    assert proposal.ops == ["refetch", "reextract"]
    assert proposal.proposed.content.title == "Second Title Bread"
    assert proposal.base.content.title == "First Title Bread"


def test_reextract_of_an_import_reparses_its_extracted_recipe(authed_api_client):
    client, _ = authed_api_client
    job = ImportApi(client).import_recipe(
        {
            "raw_recipe": {
                "title": "Imported Soup",
                "ingredients": "1 cup water\n2 carrots",
                "instructions": "Boil the water.",
            },
            "photo_ids": [],
            "extraction_method": "paprika",
        }
    )
    finished = wait_for_job_completion(ScrapeApi(client), str(job.job_id))
    assert finished.status == "completed", finished.error
    recipe_id = str(finished.recipe_id)

    ops = ops_by_id(NormalizeApi(client).list_normalize_ops(recipe_id=recipe_id))
    assert ops["reextract"].available
    assert not ops["refetch"].available

    _, proposal = run(client, recipe_id, ["reextract"])
    assert [i.item for i in proposal.proposed.content.ingredients] == [
        "water",
        "carrots",
    ]


def test_reextract_of_a_text_import_reads_its_pasted_text_again(authed_api_client):
    client, _ = authed_api_client
    recipes_api = RecipesApi(client)
    recipe_id = str(
        recipes_api.create_recipe(
            CreateRecipeRequest(
                title="My Pancakes",
                instructions="Mix and cook.",
                ingredients=[],
                raw_ingredients="1 cup flour",
                source_text="Pancakes\n1 cup flour\nMix and cook.",
            )
        ).id
    )

    ops = ops_by_id(NormalizeApi(client).list_normalize_ops(recipe_id=recipe_id))
    assert ops["reextract"].available
    assert not ops["refetch"].available

    job_id, proposal = run(client, recipe_id, ["reextract"])
    # The mock text extractor reads every text as "Text Pancakes".
    assert proposal.base.content.title == "My Pancakes"
    assert proposal.proposed.content.title == "Text Pancakes"
    assert [i.item for i in proposal.proposed.content.ingredients] == [
        "all-purpose flour",
        "butter",
        "unknowable powder",
    ]
    steps = [step.name for step in ScrapeApi(client).get_scrape(str(job_id)).steps]
    assert steps == ["extract_recipe", "parse_ingredients", "propose_normalize"]

    NormalizeApi(client).accept_normalize_proposal(recipe_id, str(job_id))
    assert recipes_api.get_recipe(recipe_id).title == "Text Pancakes"
    # The text is still the recipe's source after the new version.
    ops = ops_by_id(NormalizeApi(client).list_normalize_ops(recipe_id=recipe_id))
    assert ops["reextract"].available


def import_photo_recipe(client, image) -> tuple[str, str]:
    """Import a recipe from one photo; returns the recipe and photo ids."""
    photo_id = PhotosApi(client).upload(file=("test.png", image)).id
    job = ImportApi(client).import_from_photos(
        import_from_photos_request={"photo_ids": [photo_id]}
    )
    finished = wait_for_job_completion(ScrapeApi(client), str(job.job_id))
    assert finished.status == "completed", finished.error
    return str(finished.recipe_id), str(photo_id)


def rename(client, recipe_id: str, title: str):
    recipes_api = RecipesApi(client)
    current = recipes_api.get_recipe(recipe_id)
    recipes_api.update_recipe(
        recipe_id,
        UpdateRecipeRequest(expected_version_id=current.version_id, title=title),
    )


def test_reextract_of_a_photo_import_reads_its_photos_again(
    authed_api_client, test_image
):
    client, _ = authed_api_client
    recipe_id, _ = import_photo_recipe(client, test_image)
    rename(client, recipe_id, "My Renamed Recipe")

    ops = ops_by_id(NormalizeApi(client).list_normalize_ops(recipe_id=recipe_id))
    assert ops["reextract"].available

    job_id, proposal = run(client, recipe_id, ["reextract"])
    # The mock vision model reads every photo as "Photo Imported Recipe".
    assert proposal.base.content.title == "My Renamed Recipe"
    assert proposal.proposed.content.title == "Photo Imported Recipe"
    assert extract_ran(client, job_id)


def extract_ran(client, job_id) -> bool:
    """Whether the job ran extract_recipe, rather than reusing the import's
    output (which carries no duration)."""
    steps = {
        step.name: step for step in ScrapeApi(client).get_scrape(str(job_id)).steps
    }
    assert steps["extract_recipe"].status == "completed"
    return steps["extract_recipe"].duration_ms is not None


def test_reextract_of_a_photo_import_reparses_once_its_photos_are_gone(
    authed_api_client, test_image, database_url
):
    client, _ = authed_api_client
    recipe_id, photo_id = import_photo_recipe(client, test_image)
    conn = connect_db(database_url)
    try:
        conn.execute(
            "UPDATE photos SET deleted_at = strftime('%Y-%m-%d %H:%M:%f+00:00', 'now')"
            " WHERE id = ?",
            (photo_id,),
        )
    finally:
        conn.close()

    job_id, proposal = run(client, recipe_id, ["reextract"])
    assert proposal.proposed.content.title == "Photo Imported Recipe"
    assert not extract_ran(client, job_id)


def test_blank_source_text_is_rejected(authed_api_client):
    client, _ = authed_api_client
    with pytest.raises(ApiException) as error:
        RecipesApi(client).create_recipe(
            CreateRecipeRequest(
                title="Blank",
                instructions="Cook.",
                ingredients=[],
                source_text="   ",
            )
        )
    assert error.value.status == 400


def test_bulk_starts_one_job_per_recipe(authed_api_client):
    client, _ = authed_api_client
    typed_id = create_typed_recipe(client, "Bulk  Shrimp")
    path, url = write_fixture("Bulk Page Pie")
    try:
        scraped_id = scrape(client, url)
    finally:
        path.unlink(missing_ok=True)

    response = NormalizeApi(client).normalize_recipes(
        BulkNormalizeRequest(recipe_ids=[typed_id, scraped_id], ops=["reextract"])
    )
    results = {str(r.recipe_id): r for r in response.results}
    assert results[typed_id].job_id is None
    assert "Re-extract" in results[typed_id].error
    assert results[scraped_id].error is None
    finished = wait_for_job_completion(
        ScrapeApi(client), str(results[scraped_id].job_id)
    )
    assert finished.status == "completed", finished.error


def test_refetch_needs_only_a_source_url(authed_api_client):
    """Refetch implies reextract but rebuilds from the page it fetches, so a
    typed recipe with a source URL can refetch without a saved source."""
    client, _ = authed_api_client
    path, url = write_fixture("Linked Page Tart")
    try:
        recipe_id = str(
            RecipesApi(client)
            .create_recipe(
                CreateRecipeRequest(
                    title="Typed Tart",
                    instructions="Bake.",
                    ingredients=[make_ingredient(item="butter")],
                    source_url=url,
                )
            )
            .id
        )
        ops = ops_by_id(NormalizeApi(client).list_normalize_ops(recipe_id=recipe_id))
        assert ops["refetch"].available
        assert not ops["reextract"].available
        _, proposal = run(client, recipe_id, ["refetch"])
    finally:
        path.unlink(missing_ok=True)
    assert proposal.proposed.content.title == "Linked Page Tart"


def test_accept_keeps_tag_changes_made_after_the_run(authed_api_client):
    """Accepting copies the base version's tag links, so a tag renamed after
    the run stays renamed instead of coming back under its old name."""
    client, _ = authed_api_client
    recipes_api = RecipesApi(client)
    tags_api = TagsApi(client)
    create_typed_recipe(client, "Tag Seed", tags=["test-auto-tag"])
    recipe_id = create_typed_recipe(client, "Herb  Shrimp", tags=["weeknight"])

    job_id, proposal = run(client, recipe_id, ["normalize_title", "auto_tag"])
    assert sorted(proposal.proposed.content.tags) == ["test-auto-tag", "weeknight"]

    tag = next(t for t in tags_api.list_all_tags().tags if t.name == "weeknight")
    tags_api.rename_tag(tag.id, RenameTagRequest(name="busy-night"))
    NormalizeApi(client).accept_normalize_proposal(recipe_id, str(job_id))

    recipe = recipes_api.get_recipe(recipe_id)
    assert recipe.title == "Herb Shrimp"
    assert sorted(recipe.tags) == ["busy-night", "test-auto-tag"]
    assert "weeknight" not in [t.name for t in tags_api.list_all_tags().tags]
