"""Browser regression for adding a meal-plan recipe with only the keyboard."""

import re
import uuid

import pytest
from playwright.sync_api import Page, expect

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, RecipesApi
from ramekin_client.models import CreateRecipeRequest, SignupRequest

TITLES = ("Keyboard picker lentil stew", "Keyboard picker barley salad")


@pytest.fixture
def meal_plan_page(page: Page, api_url: str, ui_url: str) -> Page:
    configuration = Configuration(host=api_url)
    with ApiClient(configuration) as client:
        signup = AuthApi(client).signup(
            SignupRequest(
                username=f"picker_{uuid.uuid4().hex[:12]}", password="testpass123"
            )
        )
    configuration.access_token = signup.token
    with ApiClient(configuration) as client:
        recipes = RecipesApi(client)
        for title in TITLES:
            recipes.create_recipe(
                CreateRecipeRequest(title=title, instructions="Cook.", ingredients=[])
            )
    page.goto(ui_url)
    page.evaluate("token => localStorage.setItem('token', token)", signup.token)
    page.goto(f"{ui_url}/meal-plan")
    return page


@pytest.mark.parametrize("key", ["Enter", "Space"])
def test_keyboard_user_can_search_and_add_recipe(meal_plan_page: Page, key: str):
    page = meal_plan_page
    title = TITLES[0] if key == "Enter" else TITLES[1]
    slot = page.locator(".meal-slot.day-today").filter(
        has=page.get_by_role("button", name=re.compile(r"^Add Dinner on "))
    )
    add = slot.get_by_role("button", name=re.compile(r"^Add Dinner on "))
    add.focus()
    add.press("Enter")

    dialog = page.get_by_role("dialog", name=re.compile(r"^Add Dinner - "))
    expect(dialog.get_by_role("heading")).to_be_focused()

    page.keyboard.press("Tab")
    search = dialog.get_by_placeholder("Search recipes...")
    expect(search).to_be_focused()
    page.keyboard.type(title.split()[-1])
    page.keyboard.press("Enter")
    result = dialog.get_by_role("button", name=title, exact=True)
    expect(result).to_be_visible()
    expect(dialog.locator(".recipe-picker-item")).to_have_count(1)

    page.keyboard.press("Tab")
    expect(dialog.get_by_role("button", name="Search", exact=True)).to_be_focused()
    page.keyboard.press("Tab")
    expect(result).to_be_focused()
    page.keyboard.press(key)

    expect(dialog).not_to_be_visible()
    expect(slot.locator(".meal-card")).to_contain_text(title)
