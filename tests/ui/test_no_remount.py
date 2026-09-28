"""
Actions that refetch data must update the page in place, not unmount and
rebuild it. Each test tags an existing row's DOM node, performs an action that
reloads the list, and checks the same node is still on the page.
"""

import uuid

from playwright.sync_api import Locator, Page, expect

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, RecipesApi
from ramekin_client.models import (
    CreateRecipeRequest,
    Ingredient,
    Measurement,
    SignupRequest,
)


def create_user_with_tags(api_url: str, tags: list[str]) -> tuple[str, str]:
    username = f"ui_remount_{uuid.uuid4().hex[:8]}"
    password = "testpass123"

    with ApiClient(Configuration(host=api_url)) as client:
        signup = AuthApi(client).signup(
            SignupRequest(username=username, password=password)
        )

    authed_config = Configuration(host=api_url)
    authed_config.access_token = signup.token
    with ApiClient(authed_config) as client:
        RecipesApi(client).create_recipe(
            CreateRecipeRequest(
                title="Remount seed",
                instructions="Cook it",
                ingredients=[
                    Ingredient(
                        item="eggs",
                        measurements=[Measurement(amount="2", unit=None)],
                    )
                ],
                tags=tags,
            )
        )

    return username, password


def log_in(page: Page, ui_url: str, username: str, password: str) -> None:
    page.goto(ui_url)
    page.wait_for_selector("input[type='text']")
    page.fill("input[type='text']", username)
    page.fill("input[type='password']", password)
    page.click("button[type='submit']")
    page.wait_for_selector(".recipe-card")


def mark(locator: Locator) -> None:
    locator.evaluate("el => { el.__remountMarker = true; }")


def is_marked(locator: Locator) -> bool:
    return locator.evaluate("el => el.__remountMarker === true")


def test_renaming_a_tag_keeps_other_rows_mounted(page: Page, ui_url: str, api_url: str):
    username, password = create_user_with_tags(api_url, ["breakfast", "chicken"])
    log_in(page, ui_url, username, password)

    page.goto(f"{ui_url}/tags")
    chicken_row = page.locator(".tag-row").filter(has_text="chicken")
    expect(chicken_row).to_be_visible()
    mark(chicken_row)

    breakfast_row = page.locator(".tag-row").filter(has_text="breakfast")
    breakfast_row.get_by_role("button", name="Rename").click()
    page.locator(".tag-edit-input").fill("brunch")
    page.get_by_role("button", name="Save").click()

    expect(page.locator(".tag-row").filter(has_text="brunch")).to_be_visible()
    assert is_marked(chicken_row)


def test_adding_a_shopping_item_keeps_existing_rows_mounted(
    page: Page, ui_url: str, api_url: str
):
    username, password = create_user_with_tags(api_url, [])
    log_in(page, ui_url, username, password)

    page.goto(f"{ui_url}/shopping-list")
    add_input = page.locator(".shopping-add-input")
    add_input.fill("milk")
    page.get_by_role("button", name="Add", exact=True).click()
    milk_row = page.locator(".shopping-item").filter(has_text="milk")
    expect(milk_row).to_be_visible()
    mark(milk_row)

    add_input.fill("bread")
    page.get_by_role("button", name="Add", exact=True).click()

    expect(page.locator(".shopping-item").filter(has_text="bread")).to_be_visible()
    assert is_marked(milk_row)
