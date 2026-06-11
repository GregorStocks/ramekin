"""UI tests for the web settings page.

These tests require the full stack (server + UI) to be running. The seed test
user is "t" / "t".
"""

import re

from playwright.sync_api import Page, expect


def test_settings_shows_account_and_connection(logged_in_page: Page, ui_url: str):
    """The settings page shows the signed-in account and a live connection status."""
    logged_in_page.goto(f"{ui_url}/settings")
    logged_in_page.wait_for_selector(".settings-page")

    # Account info: the seed user is "t".
    username_row = logged_in_page.locator(".settings-row").filter(has_text="Username")
    expect(username_row.locator("dd")).to_have_text("t")

    # The server row shows the origin serving the app.
    expect(logged_in_page.locator(".settings-value-mono")).to_contain_text("http")

    # The connection probe succeeds against the running backend.
    expect(logged_in_page.locator(".settings-connection")).to_have_attribute(
        "data-status", "connected"
    )
    expect(logged_in_page.get_by_text("Connected")).to_be_visible()


def test_settings_links_to_tag_management(logged_in_page: Page, ui_url: str):
    """The settings page links to tag management."""
    logged_in_page.goto(f"{ui_url}/settings")
    logged_in_page.wait_for_selector(".settings-page")

    logged_in_page.get_by_role("link", name="Manage Tags").click()

    expect(logged_in_page).to_have_url(re.compile(r"/tags$"))
    logged_in_page.wait_for_selector(".tags-page")


def test_settings_reachable_from_user_menu(logged_in_page: Page):
    """The settings page is reachable from the account dropdown."""
    logged_in_page.set_viewport_size({"width": 1280, "height": 800})

    logged_in_page.get_by_role("button", name="Account menu").click()
    logged_in_page.get_by_role("link", name="Settings").click()

    expect(logged_in_page.locator(".settings-page")).to_be_visible()


def test_settings_logout(logged_in_page: Page, ui_url: str):
    """Signing out from settings clears the session and returns to login."""
    logged_in_page.goto(f"{ui_url}/settings")
    logged_in_page.wait_for_selector(".settings-page")

    logged_in_page.get_by_role("button", name="Sign Out").click()

    # Confirm in the modal dialog.
    logged_in_page.locator(".modal-content").get_by_role(
        "button", name="Sign Out"
    ).click()

    expect(logged_in_page.locator("input[type='password']")).to_be_visible()
    expect(logged_in_page).to_have_url(re.compile(r"/login$"))
    assert logged_in_page.evaluate("localStorage.getItem('token')") is None
