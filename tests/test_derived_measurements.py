"""Gram amounts are computed when a recipe is read, and never stored."""

import psycopg

from conftest import make_ingredient
from ramekin_client.api import RecipesApi
from ramekin_client.models import Ingredient, Measurement, UpdateRecipeRequest


def create(recipes, ingredients):
    return recipes.create_recipe(
        {
            "title": "Derived",
            "instructions": "Mix.",
            "ingredients": [i.to_dict() for i in ingredients],
        }
    ).id


def derived(recipe):
    return [d.to_dict() for d in recipe.derived_measurements]


def test_reads_compute_grams_without_storing_them(authed_api_client):
    client, _ = authed_api_client
    recipes = RecipesApi(client)
    ingredients = [
        make_ingredient("sugar", "2", "tbsp"),
        make_ingredient("eggs", "6"),
        make_ingredient("butter", "8", "oz"),
        make_ingredient("unicorn tears", "1", "cup"),
        Ingredient(
            item="all-purpose flour",
            measurements=[
                Measurement(amount="1", unit="cup"),
                Measurement(amount="120", unit="g"),
            ],
        ),
    ]
    recipe = recipes.get_recipe(create(recipes, ingredients))

    assert recipe.ingredients == ingredients
    assert derived(recipe) == [
        {"ingredient_index": 0, "amount": "25", "unit": "g"},
        {"ingredient_index": 2, "amount": "227", "unit": "g"},
    ]


def test_saving_a_read_recipe_stores_no_grams(authed_api_client):
    client, _ = authed_api_client
    recipes = RecipesApi(client)
    recipe_id = create(recipes, [make_ingredient("butter", "8", "oz")])
    original = recipes.get_recipe(recipe_id)

    # Editing the amount, as the clients' editors send it back.
    edited = original.ingredients[0].model_copy(
        update={"measurements": [Measurement(amount="12", unit="oz")]}
    )
    recipes.update_recipe(
        recipe_id,
        UpdateRecipeRequest(
            expected_version_id=original.version_id, ingredients=[edited]
        ),
    )
    updated = recipes.get_recipe(recipe_id)

    assert updated.ingredients[0].measurements == [Measurement(amount="12", unit="oz")]
    assert derived(updated) == [{"ingredient_index": 0, "amount": "340", "unit": "g"}]
    # An earlier version gets its own grams too.
    old = recipes.get_recipe(recipe_id, version_id=original.version_id)
    assert derived(old) == [{"ingredient_index": 0, "amount": "227", "unit": "g"}]


def test_server_startup_ran_the_gram_stripping_migration(database_url):
    with psycopg.connect(database_url) as conn:
        names = [
            row[0]
            for row in conn.execute("SELECT name FROM data_migrations").fetchall()
        ]
    assert "strip_materialized_grams" in names
