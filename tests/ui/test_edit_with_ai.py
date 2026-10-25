"""UI test for Edit with AI on a recipe page.

Uses a fresh user so accepting a proposal never touches the shared seed
user's recipes. The mock AI prefixes the title with "[Modified] " and
finishes with a summary.
"""

import uuid

from playwright.sync_api import Page, expect
from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, RecipesApi
from ramekin_client.models import CreateRecipeRequest, SignupRequest


def test_edit_with_ai_reviews_then_saves_the_proposal(
    page: Page, ui_url: str, api_url: str
):
    username = f"edit_ai_{uuid.uuid4().hex[:8]}"
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
                    title="Weeknight Soup", instructions="Simmer.", ingredients=[]
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
    page.get_by_role("button", name="Edit with AI…", exact=True).click()

    dialog = page.get_by_role("dialog", name="Edit with AI")
    run = dialog.get_by_role("button", name="Run", exact=True)
    expect(run).to_be_disabled()
    dialog.get_by_label("What would you like to change?").fill("make it vegan")
    run.click()

    expect(dialog.get_by_text("Marked the title as modified.")).to_be_visible(
        timeout=30_000
    )
    dialog.get_by_role("button", name="Accept", exact=True).click()

    expect(dialog).to_be_hidden()
    expect(page.get_by_role("heading", level=2)).to_have_text(
        "[Modified] Weeknight Soup"
    )
    with ApiClient(config) as client:
        recipe = RecipesApi(client).get_recipe(recipe_id)
    assert recipe.version_source == "ai_edit"
