"""
Smoke tests that verify the UI loads and basic navigation works.
These tests require the full stack (server + UI) to be running.
"""

from playwright.sync_api import Page, expect


def test_login_page_loads(page: Page, ui_url: str):
    """Verify the login page loads correctly."""
    page.goto(ui_url)

    # Should see login form
    expect(page.locator("input[type='text']")).to_be_visible()
    expect(page.locator("input[type='password']")).to_be_visible()
    expect(page.locator("button[type='submit']")).to_be_visible()


def test_login_and_view_cookbook(logged_in_page: Page):
    """Verify login works and cookbook page shows recipes."""
    # logged_in_page fixture handles login

    # Should have at least one recipe card (from seed data)
    recipe_cards = logged_in_page.locator(".recipe-card")
    expect(recipe_cards.first).to_be_visible()


def test_view_recipe_detail(logged_in_page: Page):
    """Verify clicking a recipe shows the detail page."""
    # Click first recipe card
    logged_in_page.locator(".recipe-card").first.click()

    # Should navigate to recipe detail page with instructions
    expect(logged_in_page.locator(".instructions")).to_be_visible()


def test_edit_recipe_page(logged_in_page: Page):
    """Verify the edit recipe page loads."""
    # Click first recipe card
    logged_in_page.locator(".recipe-card").first.click()

    # Wait for recipe page to load
    logged_in_page.wait_for_selector(".instructions")

    # Navigate to edit page
    recipe_url = logged_in_page.url
    logged_in_page.goto(f"{recipe_url}/edit")

    # Should see edit form with textarea
    expect(logged_in_page.locator("textarea").first).to_be_visible()
