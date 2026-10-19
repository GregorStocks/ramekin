"""UI test for re-parsing stored recipes from the settings page.

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


def test_settings_reparses_recipes(page: Page, ui_url: str, api_url: str):
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
        RecipesApi(client).create_recipe(
            CreateRecipeRequest(
                title="Typed Recipe",
                instructions="Cook.",
                ingredients=[
                    Ingredient(
                        item="garlic",
                        measurements=[Measurement(amount="2", unit="clove")],
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
    panel = page.get_by_role("region", name="Re-parse recipes")
    panel.get_by_role("button", name="Re-parse recipes").click()
    # A typed-in recipe has no saved scrape to read again.
    expect(panel.get_by_role("status")).to_have_text(
        "Updated 0 of 1 recipes. Skipped 0 edited by hand and 1 with no saved scrape."
    )
