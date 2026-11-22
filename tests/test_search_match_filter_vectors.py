"""Replay the shared search match/filter vectors through the real API.

shared-test-vectors/search-match-filter.json is the end-to-end contract for
search result membership and ordering: raw query strings plus recipe
documents in, matched recipe ids in final display order out. iOS replays the
same file through its local search pipeline (SearchMatchFilterSharedVector-
Tests); this harness replays it through the server so the vectors always
describe what the server actually does.

Recipes are created through the API, then the attributes a client cannot
choose through the API are set to the vector values directly in the database:
creation and version timestamps, the calorie estimate the server derives, and
the stored facts about each photo. That keeps ordering and filter cases
deterministic on both sides.
"""

import json
from datetime import datetime, timezone
from pathlib import Path

import pytest
from conftest import connect_db, wait_until
from ramekin_client.api import IngredientNamesApi, PhotosApi, RecipesApi, SyncApi
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
    "calories": SortBy.CALORIES,
}
SORT_DIR = {"asc": Direction.ASC, "desc": Direction.DESC}


@pytest.fixture(scope="module")
def vectors():
    return json.loads(VECTORS_PATH.read_text())


def _db_timestamp(iso: str) -> str:
    """An ISO-8601 vector timestamp in the server's stored text form."""
    utc = datetime.fromisoformat(iso.replace("Z", "+00:00")).astimezone(timezone.utc)
    return utc.strftime("%Y-%m-%d %H:%M:%S.%f")[:-3] + "+00:00"


def _kcal_label(kcal):
    return None if kcal is None else f"~{kcal:g} kcal"


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
    recipe_ids = {}
    conn = connect_db(database_url)
    try:
        for recipe in vectors["recipes"]:
            photo_ids = None
            if recipe["has_photo"]:
                upload = photos_api.upload(file=(f"{recipe['id']}.png", test_image))
                photo_ids = [str(upload.id)]
                photo = recipe["photo"]
                conn.execute("BEGIN IMMEDIATE")
                conn.execute(
                    "update photos set file_size = ?, width = ?, height = ? "
                    "where id = ?",
                    (
                        photo["file_size"],
                        photo["width"],
                        photo["height"],
                        str(upload.id),
                    ),
                )
                conn.execute("COMMIT")

            created = recipes_api.create_recipe(
                CreateRecipeRequest(
                    title=recipe["title"],
                    description=recipe["description"],
                    ingredients=[_ingredient(i) for i in recipe["ingredients"]],
                    instructions=recipe["instructions"],
                    notes=recipe["notes"],
                    tags=recipe["tags"],
                    rating=recipe["rating"],
                    source_name=recipe["source_name"],
                    photo_ids=photo_ids,
                )
            )
            slug_by_id[str(created.id)] = recipe["id"]
            recipe_ids[recipe["id"]] = str(created.id)

            # Creation now adds weight estimates. These vectors intentionally
            # exercise exact, user-edited ingredient representations (including
            # unusual units), so restore that corpus through the edit API.
            current = recipes_api.get_recipe(created.id)
            recipes_api.update_recipe(
                created.id,
                {
                    **current.to_dict(),
                    "expected_version_id": str(current.version_id),
                    "ingredients": [
                        _ingredient(i).to_dict() for i in recipe["ingredients"]
                    ],
                },
            )

            # The vectors carry the exact stored ingredients JSON the search
            # filter matches. It is copied from the database, never
            # hand-written; fail loudly if the stored form ever diverges.
            (match_text,) = conn.execute(
                """
                select rv.ingredients
                from recipes r join recipe_versions rv on rv.id = r.current_version_id
                where r.id = ?
                """,
                (str(created.id),),
            ).fetchone()
            assert match_text == recipe["ingredient_match_text"], recipe["id"]

        # Saving a recipe queues the names the catalog doesn't know, and the
        # name worker recomputes the stored calories of every current version
        # using a name it resolves. Let it finish with this corpus's names
        # before setting the vector values, or it overwrites them.
        names_api = IngredientNamesApi(client)
        wait_until(
            lambda: names_api.get_ingredient_names_status().pending == 0,
            timeout=60.0,
            what="the corpus's ingredient names to resolve",
        )

        for recipe in vectors["recipes"]:
            recipe_id = recipe_ids[recipe["id"]]
            conn.execute("BEGIN IMMEDIATE")
            conn.execute(
                "update recipes set created_at = ? where id = ?",
                (_db_timestamp(recipe["created_at"]), recipe_id),
            )
            conn.execute(
                """
                update recipe_versions
                set created_at = ?, kcal_per_serving = ?, kcal_per_serving_label = ?
                where id = (select current_version_id from recipes where id = ?)
                """,
                (
                    _db_timestamp(recipe["updated_at"]),
                    recipe["kcal_per_serving"],
                    _kcal_label(recipe["kcal_per_serving"]),
                    recipe_id,
                ),
            )
            conn.execute("COMMIT")
    finally:
        conn.close()

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
    ingredient_match_text; it must be the same string the search filter sees,
    and the sync must declare the normalization contract version the client
    checks before trusting local search."""
    recipes_api, slug_by_id = corpus
    expected = {r["id"]: r["ingredient_match_text"] for r in vectors["recipes"]}

    contract = json.loads(
        (REPO_ROOT / "shared-test-vectors" / "search-normalization.json").read_text()
    )
    response = SyncApi(recipes_api.api_client).sync_recipes(limit=500)
    assert response.normalization_contract_version == contract["version"]

    synced = {
        slug_by_id[str(r.id)]: r.ingredient_match_text
        for r in response.changes
        if str(r.id) in slug_by_id
    }
    assert synced == expected


def test_sync_serves_what_the_structured_filters_compare(corpus, vectors):
    """source:, calories:, photo_size: and photo_dim: run locally on iOS over
    these synced fields, so the sync must carry exactly what the server's own
    filters read."""
    recipes_api, slug_by_id = corpus
    expected = {
        r["id"]: (
            r["source_name"],
            r["kcal_per_serving"],
            _kcal_label(r["kcal_per_serving"]),
            [r["photo"]] if r["photo"] else [],
        )
        for r in vectors["recipes"]
    }

    response = SyncApi(recipes_api.api_client).sync_recipes(limit=500)
    synced = {
        slug_by_id[str(r.id)]: (
            r.source_name,
            r.kcal_per_serving,
            r.calories_per_serving,
            [
                {"file_size": p.file_size, "width": p.width, "height": p.height}
                for p in r.photos
            ],
        )
        for r in response.changes
        if str(r.id) in slug_by_id
    }
    assert synced == expected
