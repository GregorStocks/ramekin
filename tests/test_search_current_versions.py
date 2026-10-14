"""Text search only matches a recipe's current version.

The search index (`recipe_search`, an FTS5 table) holds one row per live
recipe: its current version's text, normalized. Triggers on recipes keep it
in step when `current_version_id` moves and drop the row when the recipe is
deleted, so an old version's words stop matching.
"""

import uuid

from conftest import connect_db, make_ingredient
from ramekin_client.api import RecipesApi
from ramekin_client.models import CreateRecipeRequest, UpdateRecipeRequest


def _index_rows(database_url, recipe_id):
    conn = connect_db(database_url)
    try:
        return conn.execute(
            "SELECT title, instructions FROM recipe_search"
            " WHERE rowid = (SELECT seq FROM recipes WHERE id = ?)",
            (recipe_id,),
        ).fetchall()
    finally:
        conn.close()


def test_index_follows_the_current_version(authed_api_client, database_url):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    recipe_id = str(
        recipes_api.create_recipe(
            CreateRecipeRequest(
                title="Stew",
                instructions="Simmer.",
                ingredients=[make_ingredient(item="water")],
            )
        ).id
    )
    assert _index_rows(database_url, recipe_id) == [("stew", "simmer.")]

    current = recipes_api.get_recipe(id=recipe_id)
    recipes_api.update_recipe(
        id=recipe_id,
        update_recipe_request=UpdateRecipeRequest(
            expected_version_id=str(current.version_id), title="Ragoût"
        ),
    )
    assert _index_rows(database_url, recipe_id) == [("ragout", "simmer.")]

    recipes_api.delete_recipe(id=recipe_id)
    assert _index_rows(database_url, recipe_id) == []


def test_search_skips_old_versions(authed_api_client):
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    old_token = uuid.uuid4().hex[:12]
    new_token = uuid.uuid4().hex[:12]
    recipe_id = str(
        recipes_api.create_recipe(
            CreateRecipeRequest(
                title="Stew",
                instructions=f"Stir in the {old_token}.",
                ingredients=[make_ingredient(item="water")],
            )
        ).id
    )
    current = recipes_api.get_recipe(id=recipe_id)
    recipes_api.update_recipe(
        id=recipe_id,
        update_recipe_request=UpdateRecipeRequest(
            expected_version_id=str(current.version_id),
            instructions=f"Stir in the {new_token}.",
        ),
    )

    assert recipes_api.list_recipes(q=old_token).recipes == []
    assert [str(r.id) for r in recipes_api.list_recipes(q=new_token).recipes] == [
        recipe_id
    ]
