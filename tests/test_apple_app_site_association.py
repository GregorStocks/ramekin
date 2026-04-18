import requests


def test_aasa_endpoint_serves_applinks_json(server_url):
    response = requests.get(f"{server_url}/.well-known/apple-app-site-association")
    response.raise_for_status()

    assert response.headers["content-type"].startswith("application/json")

    body = response.json()
    details = body["applinks"]["details"]
    assert len(details) == 1

    detail = details[0]
    assert detail["appIDs"] == ["32ANM8P9HJ.com.ramekin.app"]
    assert {"/": "/recipes/*"} in detail["components"]
