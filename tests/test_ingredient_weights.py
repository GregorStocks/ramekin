"""Catalog step 3, weights: a calorie estimate queues the weights the catalog
lacks for foods it knows, the LLM (mock OpenRouter) estimates them in the
background, and later estimates count the line, labeled."""

import uuid

from conftest import make_ingredient
from ramekin_client.api import IngredientNamesApi, RecipesApi
from ramekin_client.models import CreateRecipeRequest, EstimateCaloriesRequest
from test_ingredient_names import mock_answers, mock_calls, mock_fail, wait_for

# A catalog food with calories but no density or jar weight.
FOOD = "capers"


def unique_unit() -> str:
    """A unit word no other test uses: letters only, so it's asked about, and
    no trailing "s", so it's its own piece spelling."""
    token = uuid.uuid4().hex[:8].translate(str.maketrans("0123456789", "ghijklmnop"))
    return f"zq{token}"


def estimate(api: RecipesApi, amount: str, unit: str):
    return api.estimate_calories(
        EstimateCaloriesRequest(
            ingredients=[make_ingredient(FOOD, amount, unit)], scale=1
        )
    )


def create_recipe(api: RecipesApi, amount: str, unit: str) -> str:
    return api.create_recipe(
        CreateRecipeRequest(
            title=f"Recipe with {amount} {unit} {FOOD}",
            instructions="Cook.",
            ingredients=[make_ingredient(FOOD, amount, unit)],
        )
    ).id


def weight_failures(api: IngredientNamesApi, unit: str):
    return [
        f for f in api.get_ingredient_names_status().weights.failures if f.unit == unit
    ]


def test_missing_unit_weights_are_estimated_and_labeled(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    unit = unique_unit()
    first = estimate(api, "2", unit)
    assert first.lines[0].text == "Amount unclear"
    assert first.resolving
    counted = wait_for(lambda: (e := estimate(api, "2", unit)).lines[0].calories and e)
    assert counted.lines[0].text.endswith("(estimated weight)"), counted.lines[0].text
    assert not counted.resolving
    # Two of the mock's 50 g units.
    plain = api.estimate_calories(
        EstimateCaloriesRequest(
            ingredients=[make_ingredient(FOOD, "100", "g")], scale=1
        )
    )
    assert abs(counted.lines[0].calories.max - plain.lines[0].calories.max) < 1e-6
    assert mock_answers(unit) == 1


def test_missing_densities_are_estimated(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    # The catalog has no density for capers; another test may already have
    # had the shared estimate made, so only the end state is checked.
    text = wait_for(
        lambda: (
            (t := estimate(api, "2", "tbsp").lines[0].text).endswith(
                "(estimated weight)"
            )
            and t
        )
    )
    assert text.startswith("~")


def test_units_with_no_typical_weight_stay_unknown(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    unit = unique_unit() + "nosize"
    estimate(api, "2", unit)
    wait_for(lambda: mock_answers(unit) >= 1)
    later = wait_for(lambda: (e := estimate(api, "2", unit)) and not e.resolving and e)
    assert later.lines[0].text == "Amount unclear"
    assert later.lines[0].calories is None


def test_weight_failures_are_visible_and_retryable(
    authed_api_client, second_authed_api_client
):
    client, _ = authed_api_client
    api = RecipesApi(client)
    names_api = IngredientNamesApi(client)
    unit = unique_unit()
    mock_fail(unit, True)
    create_recipe(api, "2", unit)
    estimate(api, "2", unit)
    failure = wait_for(lambda: next(iter(weight_failures(names_api, unit)), None))
    assert failure.error
    assert failure.food
    assert names_api.get_ingredient_names_status().weights.failed >= 1
    assert estimate(api, "2", unit).lines[0].text == "Amount unclear"

    # Another account doesn't see it, and its retry leaves it alone.
    other_client, _ = second_authed_api_client
    theirs = IngredientNamesApi(other_client)
    assert weight_failures(theirs, unit) == []

    mock_fail(unit, False)
    assert names_api.retry_ingredient_names().queued >= 1
    wait_for(lambda: estimate(api, "2", unit).lines[0].calories)
    assert weight_failures(names_api, unit) == []
    assert mock_calls(unit) >= 2
