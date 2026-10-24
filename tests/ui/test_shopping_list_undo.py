"""Deleting from the shopping list asks nothing; a toast offers to undo it,
and the history page can undo any change."""

import re
import uuid

from playwright.sync_api import Locator, Page, expect

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi
from ramekin_client.models import SignupRequest


def sign_up_and_log_in(page: Page, ui_url: str, api_url: str) -> None:
    username = f"ui_shopping_{uuid.uuid4().hex[:8]}"
    password = "testpass123"
    with ApiClient(Configuration(host=api_url)) as client:
        AuthApi(client).signup(SignupRequest(username=username, password=password))
    page.goto(ui_url)
    page.wait_for_selector("input[type='text']")
    page.fill("input[type='text']", username)
    page.fill("input[type='password']", password)
    page.click("button[type='submit']")
    page.wait_for_url(re.compile(r"/$"))


def shopping_row(page: Page, name: str) -> Locator:
    # Match the item name exactly: each row's category <select> also
    # contains words like "Bread & Bakery".
    return page.locator(".shopping-item").filter(
        has=page.locator(".shopping-item-name", has_text=re.compile(f"^{name}$"))
    )


def add_items(page: Page, ui_url: str, *names: str) -> None:
    page.goto(f"{ui_url}/shopping-list")
    add_input = page.locator(".shopping-add-input")
    for name in names:
        add_input.fill(name)
        page.get_by_role("button", name="Add", exact=True).click()
        expect(shopping_row(page, name)).to_be_visible()


def test_clear_checked_can_be_undone_from_the_toast(
    page: Page, ui_url: str, api_url: str
):
    sign_up_and_log_in(page, ui_url, api_url)
    add_items(page, ui_url, "milk", "bread")
    shopping_row(page, "milk").locator(".shopping-checkbox").check()
    expect(page.locator(".checked-group .shopping-item")).to_have_count(1)

    page.get_by_role("button", name="Clear Checked").click()

    toast = page.locator(".shopping-undo-toast")
    expect(toast).to_contain_text("Cleared 1 item")
    expect(shopping_row(page, "milk")).to_have_count(0)
    toast.get_by_role("button", name="Undo").click()
    expect(toast).to_have_count(0)
    expect(page.locator(".checked-group").locator(".shopping-item")).to_have_count(1)
    expect(shopping_row(page, "milk")).to_be_visible()


def test_deleting_an_item_needs_no_confirmation(page: Page, ui_url: str, api_url: str):
    sign_up_and_log_in(page, ui_url, api_url)
    add_items(page, ui_url, "eggs")

    row = shopping_row(page, "eggs")
    row.hover()
    row.get_by_title("Delete item").click()

    expect(page.get_by_role("dialog")).to_have_count(0)
    expect(row).to_have_count(0)
    expect(page.locator(".shopping-undo-toast")).to_contain_text("Deleted eggs")


def test_history_page_undoes_a_change(page: Page, ui_url: str, api_url: str):
    sign_up_and_log_in(page, ui_url, api_url)
    add_items(page, ui_url, "milk")
    shopping_row(page, "milk").locator(".shopping-checkbox").check()
    expect(page.locator(".checked-group .shopping-item")).to_have_count(1)

    page.get_by_role("link", name="History").click()
    entry = page.locator(".shopping-history-entry").filter(
        has_text=re.compile("^Checked milk")
    )
    expect(entry).to_be_visible()
    expect(
        page.locator(".shopping-history-entry").filter(has_text="Added milk")
    ).to_be_visible()

    entry.get_by_role("button", name="Undo").click()
    expect(entry).to_contain_text("Undone")
    expect(
        page.locator(".shopping-history-entry").filter(has_text="Unchecked milk")
    ).to_be_visible()

    page.get_by_role("link", name="Back to List").click()
    expect(shopping_row(page, "milk")).to_be_visible()
    expect(page.locator(".checked-group")).to_have_count(0)
