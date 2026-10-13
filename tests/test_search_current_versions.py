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

# Mirrors the server's text filter in api/recipes/list.rs for one token.
SEARCH_SQL = """
select recipes.id
from recipes
join recipe_versions on recipe_versions.id = recipes.current_version_id
where recipes.user_id = %(user_id)s
  and recipes.deleted_at is null
  and recipe_versions.is_current
  and (
    f_unaccent(recipe_versions.title) ilike %(pattern)s
    or f_unaccent(recipe_versions.description) ilike %(pattern)s
    or f_unaccent(recipe_versions.instructions) ilike %(pattern)s
    or f_unaccent(recipe_versions.notes) ilike %(pattern)s
    or f_unaccent(recipe_versions.ingredients::text) ilike %(pattern)s
  )
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


def _plan_nodes(plan):
    yield plan
    for child in plan.get("Plans", []):
        yield from _plan_nodes(child)


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
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)
    token = uuid.uuid4().hex[:12]
    recipe_id = _create_with_versions(recipes_api, f"Stir in the {token}.", 3)

    assert [str(r.id) for r in recipes_api.list_recipes(q=token).recipes] == [recipe_id]

    with psycopg.connect(database_url) as conn:
        # Every version contains the token, so an index over all versions
        # would hand the heap scan three rows. On a table this small the
        # planner would rather walk recipes and fetch each current version by
        # primary key, so rule out every path except the trigram bitmaps.
        for setting in ("enable_seqscan", "enable_indexscan", "enable_nestloop"):
            conn.execute(f"set local {setting} = off")
        ((explain,),) = conn.execute(
            "explain (analyze, format json) " + SEARCH_SQL,
            {"user_id": user_id, "pattern": f"%{token}%"},
        ).fetchall()

    plan = explain[0]["Plan"]
    all_nodes = list(_plan_nodes(plan))
    assert any("_trgm" in node.get("Index Name", "") for node in all_nodes), plan
    nodes = [n for n in all_nodes if n.get("Relation Name") == "recipe_versions"]
    scanned = sum(
        node["Actual Rows"] * node["Actual Loops"]
        + node.get("Rows Removed by Index Recheck", 0)
        + node.get("Rows Removed by Filter", 0)
        for node in nodes
    )
    assert scanned == 1, plan
