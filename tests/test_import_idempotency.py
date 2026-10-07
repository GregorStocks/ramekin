"""Import idempotency keys make resubmitting an import return the original job."""

import pytest

from conftest import wait_for_job_completion
from ramekin_client.api import ImportApi, RecipesApi, ScrapeApi
from ramekin_client.exceptions import ApiException


def import_request(title, key=None):
    request = {
        "raw_recipe": {
            "title": title,
            "ingredients": "1 cup water",
            "instructions": "Boil the water.",
        },
        "photo_ids": [],
        "extraction_method": "paprika",
    }
    if key is not None:
        request["idempotency_key"] = key
    return request


def import_with_status(client, request):
    response = ImportApi(client).import_recipe_with_http_info(request)
    return response.status_code, response.data.job_id


def test_repeated_key_returns_original_job(authed_api_client):
    client, _ = authed_api_client
    status, job_id = import_with_status(client, import_request("Keyed Soup", "k1"))
    assert status == 201
    status, repeat_id = import_with_status(client, import_request("Keyed Soup", "k1"))
    assert (status, repeat_id) == (200, job_id)

    status, other_id = import_with_status(client, import_request("Keyed Soup", "k2"))
    assert status == 201 and other_id != job_id

    jobs = ImportApi(client).lookup_import_jobs(
        {"idempotency_keys": ["k1", "k2", "unknown"]}
    )
    assert {job.idempotency_key: job.job_id for job in jobs.jobs} == {
        "k1": job_id,
        "k2": other_id,
    }


def test_imports_without_keys_are_not_deduplicated(authed_api_client):
    client, _ = authed_api_client
    _, first = import_with_status(client, import_request("Unkeyed Soup"))
    _, second = import_with_status(client, import_request("Unkeyed Soup"))
    assert first != second


def test_keys_are_scoped_per_user(authed_api_client, second_authed_api_client):
    client, _ = authed_api_client
    other_client, _ = second_authed_api_client
    _, job_id = import_with_status(client, import_request("Mine", "shared"))
    status, other_id = import_with_status(
        other_client, import_request("Theirs", "shared")
    )
    assert status == 201 and other_id != job_id
    assert (
        ImportApi(other_client)
        .lookup_import_jobs({"idempotency_keys": ["shared"]})
        .jobs[0]
        .job_id
        == other_id
    )


def test_one_recipe_per_key(authed_api_client):
    client, _ = authed_api_client
    _, job_id = import_with_status(client, import_request("Single Soup", "once"))
    wait_for_job_completion(ScrapeApi(client), job_id)
    import_with_status(client, import_request("Single Soup", "once"))
    titles = [r.title for r in RecipesApi(client).list_recipes().recipes]
    assert titles.count("Single Soup") == 1


@pytest.mark.parametrize("key", ["", "x" * 256])
def test_invalid_keys_are_rejected(authed_api_client, key):
    client, _ = authed_api_client
    with pytest.raises(ApiException) as error:
        ImportApi(client).import_recipe(import_request("Bad Key", key))
    assert error.value.status == 400
    with pytest.raises(ApiException) as error:
        ImportApi(client).lookup_import_jobs({"idempotency_keys": [key]})
    assert error.value.status == 400
