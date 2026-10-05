"""Gram amounts are computed when a recipe is read, and never stored."""

import time
import uuid

import psycopg

from conftest import make_ingredient
from ramekin_client.api import RecipesApi
from ramekin_client.models import (
    EstimateCaloriesRequest,
    Ingredient,
    Measurement,
    UpdateRecipeRequest,
)


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


def test_learned_names_get_grams_but_estimates_do_not(authed_api_client):
    client, _ = authed_api_client
    recipes = RecipesApi(client)
    # Names the catalog doesn't know; saving queues them for the (mock) model,
    # which maps the sugar to a catalog entry and estimates the other.
    token = uuid.uuid4().hex[:8].translate(str.maketrans("0123456789", "ghijklmnop"))
    sugar = f"zq{token} sugar"
    estimable = f"zq{token} estimable"
    ingredients = [
        make_ingredient(sugar, "2", "tbsp"),
        make_ingredient(estimable, "1", "cup"),
    ]
    recipe_id = create(recipes, ingredients)

    def resolved():
        lines = recipes.estimate_calories(
            EstimateCaloriesRequest(ingredients=ingredients, scale=1)
        ).lines
        return all(line.text != "Not recognized" for line in lines)

    deadline = time.monotonic() + 20
    while not resolved():
        assert time.monotonic() < deadline, "names were never resolved"
        time.sleep(0.2)

    # Grams through the learned entry's density; the estimate's own 150 g/cup
    # is never used for grams.
    grams = derived(recipes.get_recipe(recipe_id))
    assert [(g["ingredient_index"], g["unit"]) for g in grams] == [(0, "g")]


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
