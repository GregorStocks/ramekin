"""UI test for the Re-normalize sheet on a recipe page.

Uses a fresh user so accepting a proposal never touches the shared seed
user's recipes. The mock AI tidies runs of spaces in titles, so a spaced-out
title gives Normalize title something to change.
"""

import uuid

from playwright.sync_api import Page, expect
from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, RecipesApi
from ramekin_client.models import CreateRecipeRequest, SignupRequest


def test_renormalize_reviews_then_saves_the_proposal(
    page: Page, ui_url: str, api_url: str
):
    username = f"renormalize_{uuid.uuid4().hex[:8]}"
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
        recipe_id = (
            RecipesApi(client)
            .create_recipe(
                CreateRecipeRequest(
                    title="Spaced    Out   Soup",
                    instructions="Simmer.",
                    ingredients=[],
                )
            )
            .id
        )

    page.goto(ui_url)
    page.wait_for_selector("input[type='text']")
    page.fill("input[type='text']", username)
    page.fill("input[type='password']", password)
    page.click("button[type='submit']")
    page.wait_for_selector(".recipe-card")

    page.goto(f"{ui_url.rstrip('/')}/recipes/{recipe_id}")
    page.locator(".recipe-more-actions > summary").click()
    page.get_by_role("button", name="Re-normalize…", exact=True).click()

    dialog = page.get_by_role("dialog", name="Re-normalize recipe")
    # A typed-in recipe has no source to re-fetch or re-extract.
    expect(dialog.get_by_role("checkbox", name="Re-fetch from source")).to_be_disabled()
    title_op = dialog.get_by_role("checkbox", name="Normalize title")
    expect(title_op).to_be_checked()
    for name in ("Generate description", "Auto-tag"):
        dialog.get_by_role("checkbox", name=name).uncheck()

    dialog.get_by_role("button", name="Run", exact=True).click()
    expect(dialog.get_by_text("Title", exact=True)).to_be_visible(timeout=30_000)
    dialog.get_by_role("button", name="Accept", exact=True).click()

    expect(dialog).to_be_hidden()
    expect(page.get_by_role("heading", level=2)).to_have_text("Spaced Out Soup")
    with ApiClient(config) as client:
        recipe = RecipesApi(client).get_recipe(recipe_id)
    assert recipe.title == "Spaced Out Soup"
    assert recipe.version_source == "normalize"
