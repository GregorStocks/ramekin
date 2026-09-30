"""Catalog step 3: names the catalog doesn't know are resolved by the LLM
(mock OpenRouter) after save and used by calorie estimates."""

import os
import threading
import time
import uuid

import pytest
import requests

from conftest import make_ingredient, wait_for_job_completion
from ramekin_client.api import (
    ImportApi,
    IngredientNamesApi,
    RecipesApi,
    ScrapeApi,
    ShoppingListApi,
)
from ramekin_client.models import (
    CreateRecipeRequest,
    CreateShoppingListItemRequest,
    CreateShoppingListRequest,
    EstimateCaloriesRequest,
    SyncCreateItem,
    SyncRequest,
    SyncUpdateItem,
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


def import_recipe(client, item: str):
    imported = ImportApi(client).import_recipe(
        {
            "raw_recipe": {
                "title": f"Imported {item}",
                "ingredients": f"100 g {item}",
                "instructions": "Cook.",
            },
            "photo_ids": [],
            "extraction_method": "paprika",
        }
    )
    return wait_for_job_completion(ScrapeApi(client), imported.job_id)


def test_resolver_failures_do_not_strand_imports(authed_api_client):
    client, _ = authed_api_client
    names_api = IngredientNamesApi(client)
    item = unique("sugar")
    mock_fail(item.lower(), True)
    job = import_recipe(client, item)
    # Imports can't be retried, so the saved recipe's job still completes;
    # the failure shows in Settings instead.
    assert job.status == "completed"
    assert job.recipe_id
    assert failures_named(names_api, item.lower())

    # A later save retries the failed name rather than reusing the failure.
    mock_fail(item.lower(), False)
    calls = mock_calls(item.lower())
    assert import_recipe(client, item).status == "completed"
    assert mock_calls(item.lower()) > calls
    assert failures_named(names_api, item.lower()) == []
    assert line_text(RecipesApi(client), item) != "Not recognized"


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


def failures_named(names_api, name: str):
    return [
        f for f in names_api.get_ingredient_names_status().failures if f.name == name
    ]


def test_status_and_retry_only_cover_your_own_names(
    authed_api_client, second_authed_api_client
):
    client, _ = authed_api_client
    other_client, _ = second_authed_api_client
    item = unique("sugar")
    mock_fail(item.lower(), True)
    create_recipe(RecipesApi(client), item)
    mine = IngredientNamesApi(client)
    wait_for(lambda: failures_named(mine, item.lower()))

    theirs = IngredientNamesApi(other_client)
    assert failures_named(theirs, item.lower()) == []
    theirs.retry_ingredient_names()
    time.sleep(0.5)
    assert failures_named(mine, item.lower()), "another account's retry left it alone"
    mock_fail(item.lower(), False)


@pytest.mark.parametrize("padding", ["", "  "])
def test_resolving_a_name_reaches_incremental_sync(authed_api_client, padding):
    client, _ = authed_api_client
    shopping = ShoppingListApi(client)
    names_api = IngredientNamesApi(client)
    item = unique("sugar")
    stored = f"{padding}{item}{padding}"
    mock_fail(item.lower(), True)
    shopping.create_items(
        CreateShoppingListRequest(items=[CreateShoppingListItemRequest(item=stored)])
    )
    wait_for(lambda: failures_named(names_api, item.lower()))
    synced = shopping.sync_items(SyncRequest())

    # Resolving after the client's last sync must mark the item changed, so
    # the next incremental sync carries its (possibly new) category.
    mock_fail(item.lower(), False)
    names_api.retry_ingredient_names()
    wait_for(
        lambda: (
            not failures_named(names_api, item.lower())
            and mock_calls(item.lower()) >= 2
        )
    )
    changes = wait_for(
        lambda: [
            c
            for c in shopping.sync_items(
                SyncRequest(last_sync_at=synced.sync_timestamp)
            ).server_changes
            if c.item == stored
        ]
    )
    assert changes[0].computed_category


def test_sync_queues_only_names_it_wrote(authed_api_client):
    client, _ = authed_api_client
    shopping = ShoppingListApi(client)
    prefix = unique("")
    written = f"{prefix}a sugar"
    rejected = f"{prefix}b sugar"
    shopping.sync_items(
        SyncRequest(
            creates=[
                SyncCreateItem(
                    client_id=uuid.uuid4(), item=written, is_checked=False, sort_order=0
                )
            ],
            updates=[
                SyncUpdateItem(id=uuid.uuid4(), expected_version=1, item=rejected)
            ],
        )
    )
    # Queued together, the two names would sort into the same batch.
    wait_for(lambda: mock_calls(written.lower()) >= 1)
    assert mock_calls(rejected.lower()) == 0


def test_create_queues_only_names_it_inserted(authed_api_client):
    client, _ = authed_api_client
    shopping = ShoppingListApi(client)
    prefix = unique("")
    written = f"{prefix}a sugar"
    rejected = f"{prefix}b sugar"
    client_id = uuid.uuid4()
    shopping.create_items(
        CreateShoppingListRequest(
            items=[CreateShoppingListItemRequest(item=written, client_id=client_id)]
        )
    )
    wait_for(lambda: mock_calls(written.lower()) >= 1)
    # A retry with the same client_id returns the existing row; its new name
    # was never saved, so it is never asked about.
    fresh = f"{prefix}c sugar"
    shopping.create_items(
        CreateShoppingListRequest(
            items=[
                CreateShoppingListItemRequest(item=rejected, client_id=client_id),
                CreateShoppingListItemRequest(item=fresh),
            ]
        )
    )
    # Queued together, the two names would sort into the same batch.
    wait_for(lambda: mock_calls(fresh.lower()) >= 1)
    assert mock_calls(rejected.lower()) == 0
