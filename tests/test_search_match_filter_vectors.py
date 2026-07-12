"""Replay the shared search match/filter vectors through the real API.

shared-test-vectors/search-match-filter.json is the end-to-end contract for
search result membership and ordering: raw query strings plus recipe
documents in, matched recipe ids in final display order out. iOS replays the
same file through its local search pipeline (SearchMatchFilterSharedVector-
Tests); this harness replays it through the server so the vectors always
describe what the server actually does.

Recipes are created through the API, then their creation and version
timestamps are set to the vector values directly in the database — the only
recipe attributes a client cannot choose through the API — so ordering cases
are deterministic on both sides.
"""

import json
from pathlib import Path

import psycopg
import pytest
from ramekin_client.api import PhotosApi, RecipesApi
from ramekin_client.models import (
    CreateRecipeRequest,
    Direction,
    Ingredient,
    Measurement,
    SortBy,
)

REPO_ROOT = Path(__file__).resolve().parents[1]
VECTORS_PATH = REPO_ROOT / "shared-test-vectors" / "search-match-filter.json"

SORT_BY = {
    "relevance": SortBy.RELEVANCE,
    "updated_at": SortBy.UPDATED_AT,
    "rating": SortBy.RATING,
    "title": SortBy.TITLE,
    "created_at": SortBy.CREATED_AT,
}
SORT_DIR = {"asc": Direction.ASC, "desc": Direction.DESC}


@pytest.fixture(scope="module")
def vectors():
    return json.loads(VECTORS_PATH.read_text())


def _ingredient(spec) -> Ingredient:
    return Ingredient(
        item=spec["item"],
        measurements=[
            Measurement(amount=m["amount"], unit=m["unit"])
            for m in spec["measurements"]
        ],
        note=spec["note"],
        section=spec["section"],
    )


@pytest.fixture
def corpus(authed_api_client, database_url, test_image, vectors):
    """Create the vector corpus for a fresh user; yields recipe id -> slug."""
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    photos_api = PhotosApi(client)

    slug_by_id = {}
    with psycopg.connect(database_url) as conn:
        for recipe in vectors["recipes"]:
            photo_ids = None
            if recipe["has_photo"]:
                upload = photos_api.upload(file=(f"{recipe['id']}.png", test_image))
                photo_ids = [str(upload.id)]

            created = recipes_api.create_recipe(
                CreateRecipeRequest(
                    title=recipe["title"],
                    description=recipe["description"],
                    ingredients=[_ingredient(i) for i in recipe["ingredients"]],
                    instructions=recipe["instructions"],
                    notes=recipe["notes"],
                    tags=recipe["tags"],
                    rating=recipe["rating"],
                    photo_ids=photo_ids,
                )
            )
            slug_by_id[str(created.id)] = recipe["id"]

            # The vectors carry the exact JSONB-to-text haystack the SQL
            # filter matches. It is generated from the database, never
            # hand-written; fail loudly if the stored form ever diverges.
            (match_text,) = conn.execute(
                """
                select rv.ingredients::text
                from recipes r join recipe_versions rv on rv.id = r.current_version_id
                where r.id = %s
                """,
                (created.id,),
            ).fetchone()
            assert match_text == recipe["ingredient_match_text"], recipe["id"]

            conn.execute(
                "update recipes set created_at = %s where id = %s",
                (recipe["created_at"], created.id),
            )
            conn.execute(
                """
                update recipe_versions set created_at = %s
                where id = (select current_version_id from recipes where id = %s)
                """,
                (recipe["updated_at"], created.id),
            )
        conn.commit()

    return recipes_api, slug_by_id


def test_match_filter_vectors(corpus, vectors):
    recipes_api, slug_by_id = corpus

    failures = []
    for case in vectors["cases"]:
        response = recipes_api.list_recipes(
            q=case["query"],
            sort_by=SORT_BY[case["sort_by"]] if case.get("sort_by") else None,
            sort_dir=SORT_DIR[case["sort_dir"]] if case.get("sort_dir") else None,
            limit=100,
        )
        actual = [slug_by_id[str(r.id)] for r in response.recipes]
        if actual != case["expected_ids"]:
            failures.append(
                f"case '{case['name']}' (query {case['query']!r}):\n"
                f"  expected {case['expected_ids']}\n"
                f"  actual   {actual}"
            )
        elif response.pagination.total != len(case["expected_ids"]):
            failures.append(
                f"case '{case['name']}': total {response.pagination.total} != "
                f"{len(case['expected_ids'])}"
            )

    assert not failures, "\n".join(failures)


def test_sync_serves_the_same_ingredient_match_text(corpus, vectors):
    """The iOS side of the vectors matches against the synced
    ingredient_match_text; it must be the same string the SQL filter sees,
    and the sync must declare the normalization contract version the client
    checks before trusting local search."""
    recipes_api, slug_by_id = corpus
    expected = {r["id"]: r["ingredient_match_text"] for r in vectors["recipes"]}

    contract = json.loads(
        (REPO_ROOT / "shared-test-vectors" / "search-normalization.json").read_text()
    )
    response = recipes_api.sync_recipes(limit=500)
    assert response.normalization_contract_version == contract["version"]

    synced = {
        slug_by_id[str(r.id)]: r.ingredient_match_text
        for r in response.recipes
        if str(r.id) in slug_by_id
    }
    assert synced == expected
