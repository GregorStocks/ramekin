"""
Smoke tests that verify the UI loads and basic navigation works.
These tests require the full stack (server + UI) to be running.
"""

import os
import re
import time
import uuid

from playwright.sync_api import Page, expect

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, RecipesApi, ScrapeApi
from ramekin_client.models import (
    CreateRecipeRequest,
    CreateScrapeRequest,
    Ingredient,
    Measurement,
    SignupRequest,
)


def wait_for_job_completion(scrape_api: ScrapeApi, job_id: str, timeout: float = 10.0):
    """Poll until a scrape job reaches a terminal state."""
    start = time.time()
    while time.time() - start < timeout:
        job = scrape_api.get_scrape(job_id)
        if job.status in ("completed", "failed"):
            return job
        time.sleep(0.1)
    raise TimeoutError(f"Job {job_id} did not complete within {timeout}s")


def create_scraped_recipe_with_enrichment(api_url: str) -> tuple[str, str, str]:
    """Create a user and scrape a recipe so version history includes enrichment."""
    username = f"ui_smoke_{uuid.uuid4().hex[:8]}"
    password = "testpass123"
    fixture_base_url = os.environ["FIXTURE_BASE_URL"]
    config = Configuration(host=api_url)

    with ApiClient(config) as client:
        auth_api = AuthApi(client)
        signup = auth_api.signup(SignupRequest(username=username, password=password))

    authed_config = Configuration(host=api_url)
    authed_config.access_token = signup.token

    with ApiClient(authed_config) as client:
        scrape_api = ScrapeApi(client)
        recipes_api = RecipesApi(client)
        recipes_api.create_recipe(
            CreateRecipeRequest(
                title="Tag seed",
                instructions="Seed user tags for scrape auto-tagging.",
                ingredients=[
                    Ingredient(
                        item="salt",
                        measurements=[Measurement(amount="1", unit="tsp")],
                    )
                ],
                tags=["test-auto-tag"],
            )
        )
        response = scrape_api.create_scrape(
            CreateScrapeRequest(url=f"{fixture_base_url}/seriouseats/rice_pilaf.html")
        )
        job = wait_for_job_completion(scrape_api, response.id)
        assert job.status == "completed"
        recipe = recipes_api.get_recipe(job.recipe_id)
        assert recipe.version_source == "enrichment"
        return username, password, recipe.id


def create_recipe_for_manual_photo(api_url: str) -> tuple[str, str, str]:
    """Create a user and recipe ready for manual AI photo generation."""
    username = f"ui_photo_{uuid.uuid4().hex[:8]}"
    password = "testpass123"
    config = Configuration(host=api_url)

    with ApiClient(config) as client:
        auth_api = AuthApi(client)
        signup = auth_api.signup(SignupRequest(username=username, password=password))

    authed_config = Configuration(host=api_url)
    authed_config.access_token = signup.token

    with ApiClient(authed_config) as client:
        recipes_api = RecipesApi(client)
        recipe = recipes_api.create_recipe(
            CreateRecipeRequest(
                title="Tomato Soup",
                description="A silky tomato soup with cream and basil.",
                instructions="Simmer tomatoes, blend smooth, and finish with cream.",
                ingredients=[
                    Ingredient(
                        item="tomatoes",
                        measurements=[Measurement(amount="2", unit="lb")],
                    ),
                    Ingredient(
                        item="cream",
                        measurements=[Measurement(amount="1", unit="cup")],
                    ),
                    Ingredient(
                        item="basil",
                        measurements=[Measurement(amount="1/4", unit="cup")],
                    ),
                ],
            )
        )
        return username, password, recipe.id


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


def test_mobile_nav_collapses_into_menu(logged_in_page: Page):
    """Verify header nav collapses behind a mobile menu toggle and shows
    primary destinations first, then housekeeping items below a divider."""
    logged_in_page.set_viewport_size({"width": 390, "height": 844})

    nav = logged_in_page.locator("#app-navigation")
    menu_toggle = logged_in_page.get_by_role("button", name="Open navigation menu")

    expect(menu_toggle).to_be_visible()
    expect(nav).not_to_be_visible()

    menu_toggle.click()

    expect(
        logged_in_page.get_by_role("button", name="Close navigation menu")
    ).to_be_visible()
    expect(nav).to_be_visible()
    expect(nav.get_by_role("link", name="Cookbook", exact=True)).to_be_visible()
    expect(nav.get_by_role("link", name="Meal Plan")).to_be_visible()
    expect(nav.get_by_role("link", name="Shopping List")).to_be_visible()
    expect(nav.get_by_role("link", name="+ New Recipe")).to_be_visible()
    expect(nav.get_by_role("link", name="Tags")).to_be_visible()
    expect(nav.get_by_role("link", name="Import")).to_be_visible()
    expect(nav.get_by_role("button", name="Export")).to_be_visible()
    expect(nav.get_by_role("button", name="Logout")).to_be_visible()

    # Primary destinations appear before the user-menu items in DOM order.
    labels = nav.locator("a, button").evaluate_all(
        "els => els.map(e => "
        "(e.textContent || e.getAttribute('aria-label') || '').trim())"
    )
    primary_idx = labels.index("Cookbook")
    tags_idx = labels.index("Tags")
    logout_idx = labels.index("Logout")
    assert primary_idx < tags_idx < logout_idx


def test_desktop_user_menu_dropdown(logged_in_page: Page):
    """Verify Tags / Import / Export / Logout live in a user menu dropdown
    on desktop and open when the account button is clicked."""
    logged_in_page.set_viewport_size({"width": 1280, "height": 800})

    nav = logged_in_page.locator("#app-navigation")
    # Primary destinations are visible without any interaction.
    expect(nav.get_by_role("link", name="Cookbook", exact=True)).to_be_visible()
    expect(nav.get_by_role("link", name="+ New Recipe")).to_be_visible()

    tags_item = nav.get_by_role("link", name="Tags")
    logout_item = nav.get_by_role("button", name="Logout")
    expect(tags_item).not_to_be_visible()
    expect(logout_item).not_to_be_visible()

    trigger = logged_in_page.get_by_role("button", name="Account menu")
    expect(trigger).to_be_visible()
    trigger.click()

    expect(tags_item).to_be_visible()
    expect(nav.get_by_role("link", name="Import")).to_be_visible()
    expect(nav.get_by_role("button", name="Export")).to_be_visible()
    expect(logout_item).to_be_visible()

    # Clicking outside the menu closes it.
    logged_in_page.locator(".app-main").click()
    expect(tags_item).not_to_be_visible()


def test_dev_footer_hidden_without_debug_param(logged_in_page: Page):
    """The build-commit / build-time footer is hidden by default."""
    expect(logged_in_page.locator(".app-footer")).to_have_count(0)


def test_dev_footer_shown_with_debug_param(logged_in_page: Page, ui_url: str):
    """Appending ?debug reveals the dev footer."""
    logged_in_page.goto(f"{ui_url}?debug")
    logged_in_page.wait_for_selector(".recipe-card")
    expect(logged_in_page.locator(".app-footer")).to_be_visible()


def test_cookbook_search_filters_as_you_type(logged_in_page: Page):
    """Verify cookbook search updates results without pressing Enter."""
    search_input = logged_in_page.get_by_placeholder("Search recipes...")
    search_input.fill("wonton")

    expect(logged_in_page).to_have_url(re.compile(r"[?&]q="))
    expect(
        logged_in_page.locator(".recipe-card h3").filter(has_text="Chicken Wonton Soup")
    ).to_have_count(1)
    expect(
        logged_in_page.locator(".recipe-card h3").filter(
            has_text="Apple Cider Caramels"
        )
    ).to_have_count(0)


def test_version_history_labels_enrichment_badges(
    page: Page, ui_url: str, api_url: str
):
    """Verify AI-enriched versions show the user-facing badge label."""
    username, password, recipe_id = create_scraped_recipe_with_enrichment(api_url)

    page.goto(ui_url)
    page.fill("input[type='text']", username)
    page.fill("input[type='password']", password)
    page.click("button[type='submit']")
    page.wait_for_selector(".recipe-card")

    page.goto(f"{ui_url}/recipes/{recipe_id}")
    page.get_by_text("Version History").click()

    expect(page.locator(".version-source-badge").first).to_have_text("AI Enriched")
    expect(
        page.locator(".version-source-badge").filter(has_text="enrichment")
    ).to_have_count(0)


def test_generate_ai_photo_from_recipe_detail(page: Page, ui_url: str, api_url: str):
    """Verify the manual AI photo action generates and displays a recipe image."""
    username, password, recipe_id = create_recipe_for_manual_photo(api_url)

    page.goto(ui_url)
    page.fill("input[type='text']", username)
    page.fill("input[type='password']", password)
    page.click("button[type='submit']")
    page.wait_for_selector(".recipe-card")

    page.goto(f"{ui_url}/recipes/{recipe_id}")
    page.get_by_role("button", name="Generate AI Photo").click()

    expect(page.locator(".recipe-photos img")).to_have_count(1)
    page.get_by_text("Version History").click()
    expect(page.locator(".version-source-badge").first).to_have_text("AI Photo")
