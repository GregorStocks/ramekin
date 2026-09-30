"""UI test for re-parsing stored ingredients from the settings page.

Uses a fresh user so re-parsing never touches the shared seed user's recipes.
"""

import uuid

from playwright.sync_api import Page, expect
from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, RecipesApi
from ramekin_client.models import (
    CreateRecipeRequest,
    Ingredient,
    Measurement,
    SignupRequest,
)


def test_settings_reparses_ingredients(page: Page, ui_url: str, api_url: str):
    username = f"reparse_{uuid.uuid4().hex[:8]}"
    password = "testpass123"
    config = Configuration(host=api_url)
    with ApiClient(config) as client:
        token = (
            AuthApi(client)
            .signup(SignupRequest(username=username, password=password))
            .token
        )
    config.access_token = token
    with ApiClient(config) as client:
        recipe = RecipesApi(client).create_recipe(
            CreateRecipeRequest(
                title="Reparse Test Recipe",
                instructions="Cook.",
                ingredients=[
                    Ingredient(
                        item="chickpeas, drained, rinsed",
                        measurements=[Measurement(amount="2", unit="cup")],
                    )
                ],
            )
        )

    page.goto(ui_url)
    page.wait_for_selector("input[type='text']")
    page.fill("input[type='text']", username)
    page.fill("input[type='password']", password)
    page.click("button[type='submit']")
    page.wait_for_selector(".recipe-card")

    page.goto(f"{ui_url.rstrip('/')}/settings")
    page.get_by_role("button", name="Re-parse ingredients").click()
    expect(page.get_by_role("status")).to_contain_text(
        "Checked 1 recipes; updated 1 (1 ingredients changed)."
    )

    page.goto(f"{ui_url.rstrip('/')}/recipes/{recipe.id}")
    ingredients = page.locator(".ingredients-list")
    expect(ingredients).to_contain_text("chickpeas")
    expect(ingredients).not_to_contain_text("chickpeas, drained")
