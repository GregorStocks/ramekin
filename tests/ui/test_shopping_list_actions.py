"""Browser regressions for shopping-list item actions that overlap a sync."""

import re
import uuid

from playwright.sync_api import Page, Route, expect

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, SyncApi
from ramekin_client.models import SignupRequest

from shopping_list_ops import create, items, sync


def open_list_with(page: Page, ui_url: str, api_url: str, *names: str) -> ApiClient:
    configuration = Configuration(host=api_url)
    with ApiClient(configuration) as client:
        signup = AuthApi(client).signup(
            SignupRequest(
                username=f"ui_shopacts_{uuid.uuid4().hex[:8]}",
                password="testpass123",
            )
        )
    configuration.access_token = signup.token
    client = ApiClient(configuration)
    sync(SyncApi(client), *(create(name) for name in names))
    page.goto(ui_url)
    page.evaluate("token => localStorage.setItem('token', token)", signup.token)
    page.goto(f"{ui_url}/shopping-list")
    return client


def checkbox(page: Page, name: str):
    return (
        page.locator(".shopping-item")
        .filter(
            has=page.locator(".shopping-item-name", has_text=re.compile(f"^{name}$"))
        )
        .locator(".shopping-checkbox")
    )


class HeldSync:
    """Holds the next shopping-list sync until the test continues it, lets
    every later one through, and counts the responses that come back."""

    def __init__(self, page: Page):
        self.page = page
        self.held: list[Route] = []
        self.responses = 0
        page.route("**/api/sync/shopping-list", self._handle)
        page.on("response", self._count)

    def _handle(self, route: Route) -> None:
        if route.request.method == "POST" and not self.held:
            self.held.append(route)
        else:
            route.continue_()

    def _count(self, response) -> None:
        if response.url.endswith("/api/sync/shopping-list"):
            self.responses += 1

    def wait_until(self, condition) -> None:
        for _ in range(200):
            if condition():
                return
            self.page.wait_for_timeout(50)
        raise AssertionError("timed out waiting for the shopping-list sync")


def test_checking_then_unchecking_before_the_sync_returns_ends_unchecked(
    page: Page, ui_url: str, api_url: str
):
    client = open_list_with(page, ui_url, api_url, "milk")
    box = checkbox(page, "milk")
    expect(box).to_be_visible()

    syncs = HeldSync(page)
    box.click()
    syncs.wait_until(lambda: syncs.held)
    box.click()
    expect(box).not_to_be_checked()
    syncs.held[0].continue_()
    # The check, then the uncheck: both must reach the server, in order.
    syncs.wait_until(lambda: syncs.responses >= 2)

    expect(page.locator(".checked-group")).to_have_count(0)
    expect(checkbox(page, "milk")).not_to_be_checked()
    with client:
        live = list(items(SyncApi(client)).values())
    assert [(i.item, i.is_checked) for i in live] == [("milk", False)]
