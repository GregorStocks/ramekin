"""UI flows that must not act on a closed or superseded state: the version
history list after a revert, the compare modal closed mid-load, and the PDF
export modal closed mid-generation."""

import base64
import json
import uuid

from playwright.sync_api import Error, Page, Route, expect

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, RecipesApi
from ramekin_client.models import (
    CreateRecipeRequest,
    Ingredient,
    Measurement,
    SignupRequest,
    UpdateRecipeRequest,
)

# A 1x1 PNG.
PNG_BYTES = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
)


def create_user_with_edited_recipe(api_url: str) -> tuple[str, str, str]:
    """A fresh user with one recipe that has two versions."""
    username = f"ui_flow_{uuid.uuid4().hex[:8]}"
    password = "testpass123"
    with ApiClient(Configuration(host=api_url)) as client:
        signup = AuthApi(client).signup(
            SignupRequest(username=username, password=password)
        )

    config = Configuration(host=api_url)
    config.access_token = signup.token
    with ApiClient(config) as client:
        recipes = RecipesApi(client)
        created = recipes.create_recipe(
            CreateRecipeRequest(
                title="Lentil Soup",
                instructions="Simmer lentils.",
                ingredients=[
                    Ingredient(
                        item="lentils",
                        measurements=[Measurement(amount="1", unit="cup")],
                    )
                ],
            )
        )
        current = recipes.get_recipe(created.id)
        recipes.update_recipe(
            created.id,
            UpdateRecipeRequest(
                expected_version_id=current.version_id, title="Red Lentil Soup"
            ),
        )
    return username, password, created.id


def log_in(page: Page, ui_url: str, username: str, password: str) -> None:
    page.goto(ui_url)
    page.fill("input[type='text']", username)
    page.fill("input[type='password']", password)
    page.click("button[type='submit']")
    page.wait_for_selector(".recipe-card")


def test_version_history_refreshes_after_revert(page: Page, ui_url: str, api_url: str):
    username, password, recipe_id = create_user_with_edited_recipe(api_url)
    log_in(page, ui_url, username, password)

    page.goto(f"{ui_url}/recipes/{recipe_id}")
    page.get_by_text("Version History").click()
    expect(page.locator(".version-count")).to_have_text("(2)")

    page.locator(".version-item:not(.current)").get_by_role(
        "button", name="Revert"
    ).click()
    page.get_by_role("dialog").get_by_role("button", name="Revert").click()

    # The open list picks up the version the revert created.
    expect(page.locator(".version-count")).to_have_text("(3)")
    expect(page.locator(".version-item")).to_have_count(3)


def test_compare_modal_closed_while_loading_stays_closed(
    page: Page, ui_url: str, api_url: str
):
    username, password, recipe_id = create_user_with_edited_recipe(api_url)
    log_in(page, ui_url, username, password)

    held: list[Route] = []

    def is_version_fetch(url: str) -> bool:
        return "version_id=" in url

    page.route(is_version_fetch, lambda route: held.append(route))

    page.goto(f"{ui_url}/recipes/{recipe_id}")
    page.get_by_text("Version History").click()
    checkboxes = page.locator(".version-compare-checkbox")
    expect(checkboxes).to_have_count(2)
    checkboxes.nth(0).check()
    checkboxes.nth(1).check()
    with page.expect_request(lambda request: is_version_fetch(request.url)):
        page.get_by_role("button", name="Compare").click()

    dialog = page.get_by_role("dialog")
    expect(dialog).to_be_visible()
    page.keyboard.press("Escape")
    expect(dialog).to_have_count(0)

    # A route whose request the page already gave up on can't be continued.
    for route in held:
        try:
            route.continue_()
        except Error:
            pass

    # The late responses must not reopen the modal.
    page.wait_for_timeout(1000)
    expect(page.get_by_role("dialog")).to_have_count(0)


def test_pdf_export_closed_while_generating_saves_nothing(
    page: Page, ui_url: str, api_url: str
):
    username, password, _recipe_id = create_user_with_edited_recipe(api_url)
    photo_id = str(uuid.uuid4())

    # Give the recipe a thumbnail, then hold the thumbnail download so the
    # generation is still running when the modal closes.
    def add_thumbnail(route: Route):
        response = route.fetch()
        body = response.json()
        for recipe in body.get("recipes", []):
            recipe["thumbnail_photo_id"] = photo_id
        route.fulfill(response=response, body=json.dumps(body))

    held: list[Route] = []
    page.route("**/api/recipes?*", add_thumbnail)
    page.route(f"**/api/photos/{photo_id}/thumbnail*", lambda route: held.append(route))

    downloads = []
    page.on("download", lambda download: downloads.append(download))

    log_in(page, ui_url, username, password)
    page.get_by_role("button", name="Select", exact=True).click()
    page.locator(".recipe-card-checkbox").first.check()
    page.get_by_role("button", name="Export to PDF").click()

    dialog = page.get_by_role("dialog")
    with page.expect_request(lambda request: "size=1200" in request.url):
        dialog.get_by_role("button", name="Generate PDF").click()
    # Cancel stays usable while generating: it abandons the generation.
    expect(dialog.get_by_role("button", name="Cancel")).to_be_enabled()
    page.keyboard.press("Escape")
    expect(page.get_by_role("dialog")).to_have_count(0)

    for route in held:
        route.fulfill(status=200, content_type="image/png", body=PNG_BYTES)
    page.unroute(f"**/api/photos/{photo_id}/thumbnail*")
    page.route(
        f"**/api/photos/{photo_id}/thumbnail*",
        lambda route: route.fulfill(
            status=200, content_type="image/png", body=PNG_BYTES
        ),
    )

    page.wait_for_timeout(1500)
    assert downloads == [], "an abandoned generation must not save a file"
    expect(page.get_by_role("dialog")).to_have_count(0)
