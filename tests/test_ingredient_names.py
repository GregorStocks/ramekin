"""Catalog step 3: names the catalog doesn't know are resolved by the LLM
(mock OpenRouter) after save and used by calorie estimates."""

import os
import threading
import time
import uuid

import psycopg
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
    # Letters only: the parser splits a token at a digit-letter boundary
    # ("ab53cd" -> "ab53 cd"), which would change the name it queues.
    token = uuid.uuid4().hex[:8].translate(str.maketrans("0123456789", "ghijklmnop"))
    return f"zq{token} {label}"


def mock_fail(name: str, fail: bool) -> None:
    port = os.environ["MOCK_OPENROUTER_PORT"]
    requests.get(
        f"http://localhost:{port}/test/ingredient-name-failure",
        params={"name": name, "fail": "true" if fail else "false"},
        timeout=10,
    ).raise_for_status()


def mock_hold(name: str, hold: bool) -> None:
    port = os.environ["MOCK_OPENROUTER_PORT"]
    requests.get(
        f"http://localhost:{port}/test/ingredient-name-hold",
        params={"name": name, "hold": "true" if hold else "false"},
        timeout=10,
    ).raise_for_status()


def mock_counts(name: str) -> dict:
    port = os.environ["MOCK_OPENROUTER_PORT"]
    response = requests.get(
        f"http://localhost:{port}/test/ingredient-name-calls",
        params={"name": name},
        timeout=10,
    )
    response.raise_for_status()
    return response.json()


def mock_calls(name: str) -> int:
    """Requests that included the name, answered or not."""
    return mock_counts(name)["calls"]


def mock_answers(name: str) -> int:
    """Valid answers sent for the name. A batch that another test's failing
    name broke is re-asked name by name, which adds a call but no answer."""
    return mock_counts(name)["answers"]


def create_recipe(api: RecipesApi, item: str) -> str:
    return api.create_recipe(
        CreateRecipeRequest(
            title=f"Recipe with {item}",
            instructions="Cook.",
            ingredients=[make_ingredient(item, "100", "g")],
        )
    ).id


def estimate(api: RecipesApi, item: str):
    return api.estimate_calories(
        EstimateCaloriesRequest(
            ingredients=[make_ingredient(item, "100", "g")], scale=1
        )
    )


def line_text(api: RecipesApi, item: str) -> str:
    return estimate(api, item).lines[0].text


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
    assert mock_answers(item.lower()) == 1


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
    assert mock_answers(item.lower()) == 1


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


def reask_state(database_url, name: str):
    with psycopg.connect(database_url, autocommit=True) as conn:
        return conn.execute(
            "SELECT reasked, reask_error FROM ingredient_name_resolutions"
            " WHERE name = %s",
            (name,),
        ).fetchone()


def test_failed_reasks_keep_the_answer_and_are_retryable(
    authed_api_client, database_url
):
    client, _ = authed_api_client
    api = RecipesApi(client)
    names_api = IngredientNamesApi(client)
    item = unique("sugar")
    name = item.lower()
    create_recipe(api, item)
    answered = wait_for(lambda: (t := line_text(api, item)) != "Not recognized" and t)

    # What startup does for answers from another model.
    mock_fail(name, True)
    with psycopg.connect(database_url, autocommit=True) as conn:
        conn.execute(
            "UPDATE ingredient_name_resolutions SET reasked = true WHERE name = %s",
            (name,),
        )
    names_api.retry_ingredient_names()  # wakes the worker

    # The invalid re-answer is recorded, not retried forever, and the earlier
    # answer is still served.
    failure = wait_for(
        lambda: next(
            (
                f
                for f in names_api.get_ingredient_names_status().failures
                if f.name == name
            ),
            None,
        ),
        timeout=60.0,
    )
    assert failure.error
    assert names_api.get_ingredient_names_status().reask_failed >= 1
    reasked, reask_error = reask_state(database_url, name)
    assert not reasked and reask_error
    assert line_text(api, item) == answered

    # Retry asks again, and a valid answer clears the failure.
    mock_fail(name, False)
    answers = mock_answers(name)
    assert names_api.retry_ingredient_names().queued >= 1
    wait_for(lambda: mock_answers(name) > answers, timeout=60.0)
    wait_for(lambda: reask_state(database_url, name) == (False, None))
    status = names_api.get_ingredient_names_status()
    assert all(f.name != name for f in status.failures)


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
                SyncRequest(cursor=synced.cursor)
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


def test_estimates_say_when_names_are_still_resolving(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    item = unique("sugar")
    mock_hold(item.lower(), True)
    try:
        create_recipe(api, item)
        wait_for(lambda: mock_calls(item.lower()) >= 1)
        pending = estimate(api, item)
        assert pending.resolving
        assert pending.lines[0].text == "Not recognized"
    finally:
        mock_hold(item.lower(), False)
    resolved = wait_for(lambda: (e := estimate(api, item)) and not e.resolving and e)
    assert resolved.lines[0].text != "Not recognized"


def test_failed_names_are_not_resolving(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    names_api = IngredientNamesApi(client)
    item = unique("sugar")
    mock_fail(item.lower(), True)
    try:
        # Reading queues it, so it's resolving until the attempt fails.
        assert estimate(api, item).resolving
        create_recipe(api, item)
        wait_for(lambda: failures_named(names_api, item.lower()))
        assert not estimate(api, item).resolving
    finally:
        mock_fail(item.lower(), False)


def test_reading_an_estimate_queues_names_never_saved(authed_api_client):
    """A recipe saved before a catalog change, or a draft, still gets its
    names resolved: reading the estimate queues them."""
    client, _ = authed_api_client
    api = RecipesApi(client)
    item = unique("flour")
    first = estimate(api, item)
    assert first.resolving
    assert first.lines[0].text == "Not recognized"
    text = wait_for(lambda: (t := line_text(api, item)) != "Not recognized" and t)
    assert text.endswith("kcal"), text


def test_reading_queues_names_past_the_per_read_limit(authed_api_client):
    """A read queues at most 50 new names but stays resolving until the rest
    are queued by later reads, so every name is eventually resolved."""
    client, _ = authed_api_client
    api = RecipesApi(client)
    items = [unique(f"flour {word}") for word in "abcdefghijklmnopqrstuvwxyz"]
    items += [unique(f"sugar {word}") for word in "abcdefghijklmnopqrstuvwxyz"]
    request = EstimateCaloriesRequest(
        ingredients=[make_ingredient(item, "100", "g") for item in items], scale=1
    )
    assert api.estimate_calories(request).resolving
    done = wait_for(
        lambda: (e := api.estimate_calories(request)) and not e.resolving and e,
        timeout=60.0,
    )
    assert all(line.text != "Not recognized" for line in done.lines)


def test_foods_no_entry_matches_are_estimated(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    item = unique("estimable fruit")
    create_recipe(api, item)
    counted = wait_for(lambda: (e := estimate(api, item)).lines[0].calories and e)
    assert counted.lines[0].text.endswith("(estimated calories)"), counted.lines[0].text
    # The mock's 200 kcal per 100 g.
    assert abs(counted.lines[0].calories.max - 200.0) < 1e-6
    status = IngredientNamesApi(client).get_ingredient_names_status()
    assert status.estimated >= 1


def test_ambiguous_names_take_a_labeled_default(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    # "cheese" could be several foods; the model picks the likely one. Shared
    # across runs, so only the end state is checked.
    text = wait_for(lambda: (t := line_text(api, "cheese")) and "(assumed " in t and t)
    assert text.startswith("~"), text
