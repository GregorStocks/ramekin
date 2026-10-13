"""Text search only scans current recipe versions.

The trigram indexes on recipe_versions are partial on `is_current`, which a
trigger on recipes keeps in step with `current_version_id`. Without that, a
common word rechecked every old version containing it before the join to
the current version threw the row away.
"""

import uuid

import psycopg
from conftest import make_ingredient
from ramekin_client.api import RecipesApi
from ramekin_client.models import CreateRecipeRequest, UpdateRecipeRequest

# The instructions arm of the server's text filter in api/recipes/list.rs.
INSTRUCTIONS_SEARCH_SQL = """
select id from recipe_versions
where is_current and f_unaccent(instructions) ilike %s
"""


def _create_with_versions(recipes_api, instructions, count):
    """Create a recipe and edit it until it has `count` versions."""
    recipe_id = str(
        recipes_api.create_recipe(
            CreateRecipeRequest(
                title="Stew",
                instructions=instructions,
                ingredients=[make_ingredient(item="water")],
            )
        ).id
    )
    for edit in range(1, count):
        current = recipes_api.get_recipe(id=recipe_id)
        recipes_api.update_recipe(
            id=recipe_id,
            update_recipe_request=UpdateRecipeRequest(
                expected_version_id=str(current.version_id),
                title=f"Stew v{edit + 1}",
            ),
        )
    return recipe_id


def _current_flags(conn, recipe_id):
    rows = conn.execute(
        """
        select recipe_versions.id = recipes.current_version_id,
               recipe_versions.is_current
        from recipe_versions
        join recipes on recipes.id = recipe_versions.recipe_id
        where recipes.id = %s
        """,
        (recipe_id,),
    ).fetchall()
    return rows


def test_is_current_follows_current_version(authed_api_client, database_url):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    recipe_id = _create_with_versions(recipes_api, "Simmer.", 3)

    with psycopg.connect(database_url) as conn:
        rows = _current_flags(conn, recipe_id)
        assert len(rows) == 3
        assert all(is_pointed_at == is_current for is_pointed_at, is_current in rows)

        recipes_api.delete_recipe(id=recipe_id)
        assert _current_flags(conn, recipe_id) == rows


def test_search_skips_old_versions(authed_api_client, database_url):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    token = uuid.uuid4().hex[:12]
    recipe_id = _create_with_versions(recipes_api, f"Stir in the {token}.", 3)

    assert [str(r.id) for r in recipes_api.list_recipes(q=token).recipes] == [recipe_id]

    with psycopg.connect(database_url) as conn:
        # Keep the tiny test table from planning a sequential scan instead.
        for setting in ("enable_seqscan", "enable_indexscan"):
            conn.execute(f"set local {setting} = off")
        ((explain,),) = conn.execute(
            "explain (analyze, format json) " + INSTRUCTIONS_SEARCH_SQL,
            (f"%{token}%",),
        ).fetchall()

    # Every version contains the token, so an index over all versions would
    # hand the heap three live rows to recheck and filter out. The partial
    # index only holds current versions. Versions that stopped being current
    # can leave dead index entries until vacuum, so count what the heap
    # examined rather than the index scan's raw tuple ids.
    plan = explain[0]["Plan"]
    assert plan["Node Type"] == "Bitmap Heap Scan", plan
    (index_scan,) = plan["Plans"]
    assert index_scan["Index Name"] == "idx_recipe_versions_instructions_trgm", plan
    assert "Index Cond" in index_scan, plan
    examined = (
        plan["Actual Rows"]
        + plan.get("Rows Removed by Index Recheck", 0)
        + plan.get("Rows Removed by Filter", 0)
    )
    assert examined == 1, plan
