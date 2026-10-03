"""Catalog step 3, weights: a calorie estimate queues the weights the catalog
lacks for foods it knows, the LLM (mock OpenRouter) estimates them in the
background, and later estimates count the line, labeled."""

import json
import random
from pathlib import Path

from conftest import make_ingredient
from ramekin_client.api import IngredientNamesApi, RecipesApi
from ramekin_client.models import CreateRecipeRequest, EstimateCaloriesRequest
from test_ingredient_names import mock_calls, mock_fail, unique
from test_ingredient_names import wait_for as wait_briefly

USDA = Path(__file__).resolve().parents[1] / "ramekin-core/src/catalog/data/usda.json"

# Catalog foods with calories but no density and no piece weights. Estimates
# are shared and outlive a test run, so each test weighs foods drawn at random
# here and checks they're still unasked, the way name tests use unique names.
UNWEIGHED_FOODS = [
    food["description"]
    for food in json.loads(USDA.read_text())["foods"]
    if food["kcal_per_100g"]
    and not food["portions"]
    and not food["grams_per_cup"]
    and not food["description"].startswith("spices")
    # "x or y" descriptions add an alternatives label to the line.
    and " or " not in food["description"]
    and "/" not in food["description"]
]

# Units the mock answers specially: "handful" has no typical weight, and the
# failure test breaks "bottle". Other tests leave them out.
SPECIAL_UNITS = {"handful", "bottle"}


def wait_for(predicate):
    """Weights share the worker, batch lock, and rate-limited client with
    ingredient names, whose tests hold and break batches on purpose, so give
    the background pass more room than a name test needs."""
    return wait_briefly(predicate, timeout=60.0)


def request(food: str, units: list[str], amount: str = "2") -> EstimateCaloriesRequest:
    return EstimateCaloriesRequest(
        ingredients=[make_ingredient(food, amount, unit) for unit in units], scale=1
    )


def unasked_food(api: RecipesApi, units: list[str]) -> str:
    """A catalog food none of whose `units` has an estimate yet: every line
    unknown, and the estimate waiting on them. Asking queues them."""
    for food in random.sample(UNWEIGHED_FOODS, 20):
        result = api.estimate_calories(request(food, units))
        if result.resolving and all(line.calories is None for line in result.lines):
            return food
    raise AssertionError("no unasked catalog food found")


def weight_failures(api: IngredientNamesApi, food: str, unit: str):
    return [
        f
        for f in api.get_ingredient_names_status().weights.failures
        if f.food == food and f.unit == unit
    ]


def test_missing_unit_weights_are_estimated_and_labeled(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    food = unasked_food(api, ["jar"])
    counted = wait_for(
        lambda: (
            (e := api.estimate_calories(request(food, ["jar"]))).lines[0].calories and e
        )
    )
    assert counted.lines[0].text.endswith("(estimated weight)"), counted.lines[0].text
    assert not counted.resolving
    # Two of the mock's 50 g units.
    plain = api.estimate_calories(request(food, ["g"], amount="100"))
    assert abs(counted.lines[0].calories.max - plain.lines[0].calories.max) < 1e-6


def test_missing_densities_are_estimated(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    food = unasked_food(api, ["tbsp"])
    text = wait_for(
        lambda: (
            (
                t := api.estimate_calories(request(food, ["tbsp"])).lines[0].text
            ).endswith("(estimated weight)")
            and t
        )
    )
    assert text.startswith("~")


def test_units_with_no_typical_weight_stay_unknown(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    food = unasked_food(api, ["handful"])
    later = wait_for(
        lambda: (
            (e := api.estimate_calories(request(food, ["handful"])))
            and not e.resolving
            and e
        )
    )
    assert later.lines[0].text == "Amount unclear"
    assert later.lines[0].calories is None


def test_weight_failures_are_visible_and_retryable(
    authed_api_client, second_authed_api_client
):
    client, _ = authed_api_client
    api = RecipesApi(client)
    names_api = IngredientNamesApi(client)
    unit = "bottle"
    # A food the model estimated (no catalog entry) is weighed by its stand-in
    # id, and its weights are in the account's scope too.
    estimated = unique("estimable soda")
    estimated_id = f"estimated food: {estimated.lower()}"
    mock_fail(unit, True)
    try:
        food = unasked_food(api, [unit])
        api.create_recipe(
            CreateRecipeRequest(
                title=f"Recipe with 2 {unit}s of {food}",
                instructions="Cook.",
                ingredients=[
                    make_ingredient(food, "2", unit),
                    make_ingredient(estimated, "2", unit),
                ],
            )
        )
        failure = wait_for(
            lambda: next(iter(weight_failures(names_api, food, unit)), None)
        )
        assert failure.error
        # Its name resolves to an estimate first; reading then queues the
        # bottle weight, which fails like the catalog food's.
        wait_for(
            lambda: (
                api.estimate_calories(request(estimated, [unit]))
                and weight_failures(names_api, estimated_id, unit)
            )
        )
        assert names_api.get_ingredient_names_status().weights.failed >= 1
        line = api.estimate_calories(request(food, [unit])).lines[0]
        assert line.text == "Amount unclear"

        # Another account doesn't see it.
        other_client, _ = second_authed_api_client
        assert weight_failures(IngredientNamesApi(other_client), food, unit) == []
    finally:
        mock_fail(unit, False)

    assert names_api.retry_ingredient_names().queued >= 2
    wait_for(lambda: api.estimate_calories(request(food, [unit])).lines[0].calories)
    wait_for(
        lambda: api.estimate_calories(request(estimated, [unit])).lines[0].calories
    )
    assert weight_failures(names_api, food, unit) == []
    assert weight_failures(names_api, estimated_id, unit) == []
    assert mock_calls(unit) >= 2


def test_requests_with_many_gaps_queue_them_all(authed_api_client):
    """A request queues at most 50 new gaps, but keeps `resolving` until the
    rest are queued on later polls, so every line is eventually weighed."""
    client, _ = authed_api_client
    api = RecipesApi(client)
    units = [
        "bag", "ball", "bar", "block", "box", "breast", "bulb", "bunch", "can",
        "carton", "chop", "clove", "container", "cube", "ear", "envelope",
        "extra large", "fillet", "head", "heart", "jar", "jumbo", "knob", "large",
        "leaf", "leg", "link", "loaf", "medium", "package", "packet", "piece",
        "pouch", "rib", "ring", "roll", "sheet", "slab", "slice", "small", "spear",
        "sprig", "stalk", "steak", "stem", "stick", "strip", "thigh", "tube", "tub",
        "wedge", "whole", "wing",
    ]  # fmt: skip
    assert len(units) > 50 and not SPECIAL_UNITS & set(units)
    food = unasked_food(api, units)
    done = wait_for(
        lambda: (
            (e := api.estimate_calories(request(food, units)))
            and all(line.calories for line in e.lines)
            and e
        )
    )
    assert not done.resolving
    assert all(line.text.endswith("(estimated weight)") for line in done.lines)


def test_saved_recipes_get_weights_without_being_viewed(authed_api_client):
    """Once a saved recipe's name is answered, its weight gaps are queued by
    the worker's sweep, without anyone reading the recipe's estimate."""
    client, _ = authed_api_client
    names_api = IngredientNamesApi(client)
    # A model-estimated food (unique to this test) with no weight for sheets.
    estimated = unique("estimable wrapper")
    RecipesApi(client).create_recipe(
        CreateRecipeRequest(
            title=f"Recipe with {estimated}",
            instructions="Cook.",
            ingredients=[make_ingredient(estimated, "2", "sheets")],
        )
    )

    def asked():
        weights = names_api.get_ingredient_names_status().weights
        rows = (
            weights.estimated
            + weights.no_typical_weight
            + weights.pending
            + weights.failed
        )
        return rows >= 1

    wait_for(asked)
