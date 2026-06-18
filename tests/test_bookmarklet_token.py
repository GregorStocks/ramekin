"""End-to-end tests for the long-lived, scope-restricted bookmarklet token.

The bookmarklet embeds a token that is minted differently from a login session
(`POST /api/users/bookmarklet-token`) and is restricted to exactly the
endpoints the capture flow needs. These tests pin both the minting and the
scope enforcement.
"""

import requests


def auth(token: str) -> dict:
    return {"Authorization": f"Bearer {token}"}


def mint_bookmarklet_token(server_url: str, session_token: str) -> str:
    """Mint a bookmarklet token using a full session token."""
    response = requests.post(
        f"{server_url}/api/users/bookmarklet-token",
        headers=auth(session_token),
    )
    response.raise_for_status()
    return response.json()["token"]


class TestBookmarkletTokenMint:
    def test_mint_returns_token(self, authed_api_client, server_url):
        client, _ = authed_api_client
        session_token = client.configuration.access_token

        response = requests.post(
            f"{server_url}/api/users/bookmarklet-token",
            headers=auth(session_token),
        )

        assert response.status_code == 201
        assert response.json()["token"]

    def test_mint_requires_auth(self, server_url):
        response = requests.post(f"{server_url}/api/users/bookmarklet-token")
        assert response.status_code == 401

    def test_minting_again_does_not_invalidate_old_token(
        self, authed_api_client, server_url
    ):
        """Arbitrarily many tokens coexist, so older bookmarklets keep working."""
        client, _ = authed_api_client
        session_token = client.configuration.access_token

        first = mint_bookmarklet_token(server_url, session_token)
        second = mint_bookmarklet_token(server_url, session_token)
        assert first != second

        # The first token still authenticates after a second is minted.
        response = requests.get(f"{server_url}/api/users/me", headers=auth(first))
        assert response.status_code == 200


class TestBookmarkletTokenScope:
    """A bookmarklet token may reach only the capture flow's endpoints."""

    def test_allows_me_preflight(self, authed_api_client, server_url):
        client, _ = authed_api_client
        token = mint_bookmarklet_token(server_url, client.configuration.access_token)

        response = requests.get(f"{server_url}/api/users/me", headers=auth(token))
        assert response.status_code == 200

    def test_allows_capture_and_status_poll(self, authed_api_client, server_url):
        client, _ = authed_api_client
        token = mint_bookmarklet_token(server_url, client.configuration.access_token)

        capture = requests.post(
            f"{server_url}/api/scrape/capture",
            json={"html": "<html></html>", "source_url": "https://example.com/r"},
            headers=auth(token),
        )
        assert capture.status_code == 201
        job_id = capture.json()["id"]

        status = requests.get(f"{server_url}/api/scrape/{job_id}", headers=auth(token))
        assert status.status_code == 200

    def test_forbids_other_endpoints(self, authed_api_client, server_url):
        client, _ = authed_api_client
        token = mint_bookmarklet_token(server_url, client.configuration.access_token)

        response = requests.get(f"{server_url}/api/recipes", headers=auth(token))
        assert response.status_code == 403

    def test_forbids_minting_more_tokens(self, authed_api_client, server_url):
        """A leaked bookmarklet token must not be able to mint further tokens."""
        client, _ = authed_api_client
        token = mint_bookmarklet_token(server_url, client.configuration.access_token)

        response = requests.post(
            f"{server_url}/api/users/bookmarklet-token", headers=auth(token)
        )
        assert response.status_code == 403
