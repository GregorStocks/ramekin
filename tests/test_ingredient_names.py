"""Catalog step 3: names the catalog doesn't know are resolved by the LLM
(mock OpenRouter) after save and used by calorie estimates."""

import os
import threading
import time
import uuid

import requests

from conftest import make_ingredient
from ramekin_client.api import IngredientNamesApi, RecipesApi, ShoppingListApi
from ramekin_client.models import (
    CreateRecipeRequest,
    CreateShoppingListItemRequest,
    CreateShoppingListRequest,
    EstimateCaloriesRequest,
)


def unique(label: str) -> str:
    return f"zq{uuid.uuid4().hex[:8]} {label}"


def mock_fail(name: str, fail: bool) -> None:
    port = os.environ["MOCK_OPENROUTER_PORT"]
    requests.get(
        f"http://localhost:{port}/test/ingredient-name-failure",
        params={"name": name, "fail": "true" if fail else "false"},
        timeout=10,
    ).raise_for_status()


def mock_calls(name: str) -> int:
    port = os.environ["MOCK_OPENROUTER_PORT"]
    response = requests.get(
        f"http://localhost:{port}/test/ingredient-name-calls",
        params={"name": name},
        timeout=10,
    )
    response.raise_for_status()
    return response.json()["calls"]


def create_recipe(api: RecipesApi, item: str) -> str:
    return api.create_recipe(
        CreateRecipeRequest(
            title=f"Recipe with {item}",
            instructions="Cook.",
            ingredients=[make_ingredient(item, "100", "g")],
        )
    ).id


def line_text(api: RecipesApi, item: str) -> str:
    estimate = api.estimate_calories(
        EstimateCaloriesRequest(
            ingredients=[make_ingredient(item, "100", "g")], scale=1
        )
    )
    return estimate.lines[0].text


def wait_for(predicate, timeout: float = 20.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(0.2)
    raise AssertionError("timed out")


def test_saved_unknown_names_are_resolved_and_counted(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    item = unique("sugar")
    assert line_text(api, item) == "Not recognized"
    create_recipe(api, item)
    text = wait_for(lambda: (t := line_text(api, item)) != "Not recognized" and t)
    assert text.endswith("kcal"), text
    assert mock_calls(item.lower()) == 1


def test_unknowable_names_stay_unknown(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    item = unique("unknowable sugar")
    create_recipe(api, item)
    wait_for(lambda: mock_calls(item.lower()) >= 1)
    time.sleep(0.5)
    assert line_text(api, item) == "Not recognized"


def test_concurrent_saves_resolve_a_name_once(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    item = unique("flour")
    threads = [
        threading.Thread(target=create_recipe, args=(api, item)) for _ in range(4)
    ]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    wait_for(lambda: line_text(api, item) != "Not recognized")
    assert mock_calls(item.lower()) == 1


def test_failures_are_visible_and_retryable(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    names_api = IngredientNamesApi(client)
    item = unique("sugar")
    mock_fail(item.lower(), True)
    create_recipe(api, item)
    failure = wait_for(
        lambda: next(
            (
                f
                for f in names_api.get_ingredient_names_status().failures
                if f.name == item.lower()
            ),
            None,
        )
    )
    assert failure.error
    assert line_text(api, item) == "Not recognized"

    mock_fail(item.lower(), False)
    assert names_api.retry_ingredient_names().queued >= 1
    wait_for(lambda: line_text(api, item) != "Not recognized")
    status = names_api.get_ingredient_names_status()
    assert all(f.name != item.lower() for f in status.failures)


def test_shopping_list_items_are_resolved(authed_api_client):
    client, _ = authed_api_client
    item = unique("sugar")
    ShoppingListApi(client).create_items(
        CreateShoppingListRequest(items=[CreateShoppingListItemRequest(item=item)])
    )
    wait_for(lambda: mock_calls(item.lower()) >= 1)


def test_warm_queues_nothing_new_for_saved_recipes(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    item = unique("sugar")
    create_recipe(api, item)
    wait_for(lambda: line_text(api, item) != "Not recognized")
    # Saving already queued the name, so warming finds nothing new.
    assert IngredientNamesApi(client).warm_ingredient_names().queued == 0
