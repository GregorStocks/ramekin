"""
Tests for the recipe-view scale control: scaling ingredient amounts and
the Serves: line, plus the integration with the Add to Shopping List flow.
"""

import uuid
from typing import List

import pytest
from playwright.sync_api import Page, expect

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, RecipesApi, ShoppingListApi
from ramekin_client.models import (
    CreateRecipeRequest,
    Ingredient,
    Measurement,
    SignupRequest,
)


# Each ingredient covers one representative amount shape. Order matters because
# the test asserts on the rendered list by index.
SCALE_TEST_INGREDIENTS: List[Ingredient] = [
    Ingredient(item="flour", measurements=[Measurement(amount="2", unit="cups")]),
    Ingredient(item="sugar", measurements=[Measurement(amount="1/2", unit="cup")]),
    Ingredient(
        item="butter", measurements=[Measurement(amount="1 1/2", unit="sticks")]
    ),
    Ingredient(item="milk", measurements=[Measurement(amount="2.5", unit="cups")]),
    Ingredient(item="eggs", measurements=[Measurement(amount="3", unit=None)]),
    Ingredient(item="salt", measurements=[Measurement(amount="to taste", unit=None)]),
    Ingredient(item="bay leaves", measurements=[Measurement(amount="6-8", unit=None)]),
]


def _sign_up(api_url: str) -> tuple[str, str, str]:
    """Sign up a fresh user and return (username, password, token)."""
    username = f"scale_{uuid.uuid4().hex[:8]}"
    password = "testpass123"
    config = Configuration(host=api_url)
    with ApiClient(config) as client:
        auth_api = AuthApi(client)
        signup = auth_api.signup(SignupRequest(username=username, password=password))
    return username, password, signup.token


def _authed_client(api_url: str, token: str) -> ApiClient:
    config = Configuration(host=api_url)
    config.access_token = token
    return ApiClient(config)


@pytest.fixture
def scale_recipe(api_url: str, ui_url: str, page: Page):
    """Create a user with a known scale-test recipe and log the page in.

    Yields (recipe_id, token) so tests can hit the API directly.
    """
    username, password, token = _sign_up(api_url)
    with _authed_client(api_url, token) as client:
        recipes_api = RecipesApi(client)
        recipe = recipes_api.create_recipe(
            CreateRecipeRequest(
                title="Scale Test Recipe",
                instructions="Mix and bake.",
                ingredients=SCALE_TEST_INGREDIENTS,
                servings="4",
            )
        )

    page.goto(ui_url)
    page.wait_for_selector("input[type='text']")
    page.fill("input[type='text']", username)
    page.fill("input[type='password']", password)
    page.click("button[type='submit']")
    page.wait_for_selector(".recipe-card")

    page.goto(f"{ui_url.rstrip('/')}/recipes/{recipe.id}")
    page.wait_for_selector(".ingredients-list")

    yield recipe.id, token


def _amount_texts(page: Page) -> List[str]:
    """Return the visible primary-amount text for each rendered ingredient row."""
    return page.locator(".ingredients-list li .amount").all_text_contents()


def test_recipe_loads_with_original_amounts(scale_recipe, page: Page):
    _recipe_id, _token = scale_recipe
    amounts = _amount_texts(page)
    assert "2" in amounts
    assert "1/2" in amounts
    assert "1 1/2" in amounts
    assert "2.5" in amounts
    assert "3" in amounts
    expect(page.locator(".recipe-metadata")).to_contain_text("Serves:")
    expect(page.locator(".recipe-metadata")).to_contain_text("4")


def test_clicking_preset_updates_url_and_active_state(scale_recipe, page: Page):
    _recipe_id, _token = scale_recipe
    assert "scale=" not in page.url
    one_x = page.locator(".scale-preset", has_text="1×")
    expect(one_x).to_have_class("scale-preset active")

    page.locator(".scale-preset", has_text="2×").click()
    page.wait_for_url(lambda url: "scale=2" in url)
    expect(page.locator(".scale-preset", has_text="2×")).to_have_class(
        "scale-preset active"
    )
    expect(one_x).not_to_have_class("scale-preset active")

    one_x.click()
    page.wait_for_url(lambda url: "scale=" not in url)
    expect(one_x).to_have_class("scale-preset active")


def test_custom_input_overrides_presets(scale_recipe, page: Page):
    _recipe_id, _token = scale_recipe
    custom = page.locator(".scale-custom-input")
    custom.fill("1.5")
    custom.press("Enter")
    page.wait_for_url(lambda url: "scale=1.5" in url)
    for preset_label in ["¼×", "½×", "1×", "2×", "3×"]:
        expect(page.locator(".scale-preset", has_text=preset_label)).not_to_have_class(
            "scale-preset active"
        )


def test_scale_2x_doubles_amounts(scale_recipe, page: Page):
    _recipe_id, _token = scale_recipe
    page.locator(".scale-preset", has_text="2×").click()
    page.wait_for_url(lambda url: "scale=2" in url)

    amounts = _amount_texts(page)
    # 2 → 4, 1/2 → 1, 1 1/2 → 3, 2.5 → 5, 3 → 6
    assert "4" in amounts
    assert "1" in amounts
    assert "3" in amounts
    assert "5" in amounts
    assert "6" in amounts
    # Unparseable amounts pass through unchanged.
    page_text = page.locator(".ingredients-list").inner_text()
    assert "to taste" in page_text
    assert "6-8" in page_text
    # Serves: 4 → 8
    expect(page.locator(".recipe-metadata")).to_contain_text("8")
    # Badge is shown (one near Ingredients heading, one near Serves: line).
    assert page.locator(".scale-badge").count() == 2
    expect(page.locator(".scale-badge").first).to_contain_text("scaled 2×")


def test_scale_half_halves_amounts(scale_recipe, page: Page):
    _recipe_id, _token = scale_recipe
    page.locator(".scale-preset", has_text="½×").click()
    page.wait_for_url(lambda url: "scale=0.5" in url)

    amounts = _amount_texts(page)
    # 2 → 1, 1/2 → 1/4, 1 1/2 → 0.75, 2.5 → 1.25, 3 → 1.5
    assert "1" in amounts
    assert "1/4" in amounts
    assert "0.75" in amounts
    assert "1.25" in amounts
    assert "1.5" in amounts


def test_clicking_1x_clears_badge(scale_recipe, page: Page):
    _recipe_id, _token = scale_recipe
    page.locator(".scale-preset", has_text="2×").click()
    page.wait_for_url(lambda url: "scale=2" in url)
    assert page.locator(".scale-badge").count() == 2

    page.locator(".scale-preset", has_text="1×").click()
    page.wait_for_url(lambda url: "scale=" not in url)
    expect(page.locator(".scale-badge")).to_have_count(0)


def test_shopping_list_uses_scaled_amounts(scale_recipe, api_url, page: Page):
    _recipe_id, token = scale_recipe

    page.locator(".scale-preset", has_text="2×").click()
    page.wait_for_url(lambda url: "scale=2" in url)
    page.locator("button", has_text="Add to Shopping List").click()
    page.wait_for_selector(".add-shopping-modal")

    modal_text = page.locator(".add-shopping-modal").inner_text()
    assert "4 cups flour" in modal_text  # 2 → 4
    assert "1 cup sugar" in modal_text  # 1/2 → 1

    page.locator(".modal-actions button.btn-primary").click()
    page.wait_for_selector(".add-shopping-success")

    config = Configuration(host=api_url)
    config.access_token = token
    with ApiClient(config) as client:
        items = ShoppingListApi(client).list_items().items

    by_item = {it.item: it.amount for it in items}
    assert by_item["flour"] == "4 cups"
    assert by_item["sugar"] == "1 cup"
    assert by_item["butter"] == "3 sticks"
    assert by_item["milk"] == "5 cups"
    assert by_item["eggs"] == "6"
    assert by_item["salt"] == "to taste"
    assert by_item["bay leaves"] == "6-8"
