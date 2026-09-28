"""
Actions that refetch data must update the page in place, not unmount and
rebuild it: the row tests tag an existing DOM node, perform an action that
reloads the list, and check the same node is still on the page. Navigating to
a *different* recipe or version is the opposite case: the old content must not
linger while the new one loads.
"""

import re
import uuid

from playwright.sync_api import Locator, Page, Route, expect

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, RecipesApi
from ramekin_client.models import (
    CreateRecipeRequest,
    Ingredient,
    LoginRequest,
    Measurement,
    SignupRequest,
    UpdateRecipeRequest,
)


def create_user_with_recipes(
    api_url: str, titles: list[str], tags: list[str]
) -> tuple[str, str, list[str]]:
    username = f"ui_remount_{uuid.uuid4().hex[:8]}"
    password = "testpass123"

    with ApiClient(Configuration(host=api_url)) as client:
        signup = AuthApi(client).signup(
            SignupRequest(username=username, password=password)
        )

    authed_config = Configuration(host=api_url)
    authed_config.access_token = signup.token
    recipe_ids = []
    with ApiClient(authed_config) as client:
        for title in titles:
            recipe = RecipesApi(client).create_recipe(
                CreateRecipeRequest(
                    title=title,
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
            recipe_ids.append(recipe.id)

    return username, password, recipe_ids


def create_user_with_tags(api_url: str, tags: list[str]) -> tuple[str, str]:
    username, password, _ = create_user_with_recipes(api_url, ["Remount seed"], tags)
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


def test_checking_a_shopping_item_keeps_other_rows_mounted(
    page: Page, ui_url: str, api_url: str
):
    username, password = create_user_with_tags(api_url, [])
    log_in(page, ui_url, username, password)

    def shopping_row(name: str) -> Locator:
        # Match the item name exactly: each row's category <select> also
        # contains words like "Bread & Bakery".
        return page.locator(".shopping-item").filter(
            has=page.locator(".shopping-item-name", has_text=re.compile(f"^{name}$"))
        )

    page.goto(f"{ui_url}/shopping-list")
    add_input = page.locator(".shopping-add-input")
    for name in ["milk", "bread"]:
        add_input.fill(name)
        page.get_by_role("button", name="Add", exact=True).click()
        expect(shopping_row(name)).to_be_visible()

    bread_row = shopping_row("bread")
    mark(bread_row)

    shopping_row("milk").locator(".shopping-checkbox").check()

    expect(page.locator(".checked-group .shopping-item")).to_have_count(1)
    assert is_marked(bread_row)


def test_navigating_to_another_recipe_hides_the_previous_one(
    page: Page, ui_url: str, api_url: str
):
    """The page is reused across recipes, but must not keep showing the old
    recipe (whose actions would already target the new id) while the next
    one loads."""
    username, password, (first_id, second_id) = create_user_with_recipes(
        api_url, ["Applesauce", "Zucchiniloaf"], []
    )
    log_in(page, ui_url, username, password)

    page.goto(f"{ui_url}/recipes/{first_id}?randomQ=Zucchiniloaf")
    expect(page.get_by_role("heading", name="Applesauce")).to_be_visible()

    pending: list[Route] = []
    page.route(f"**/api/recipes/{second_id}", lambda route: pending.append(route))
    page.get_by_role("button", name="Next Random").click()

    expect(page.get_by_text("Loading recipe...")).to_be_visible()
    expect(page.get_by_role("heading", name="Applesauce")).not_to_be_visible()

    for route in pending:
        route.continue_()
    page.unroute(f"**/api/recipes/{second_id}")
    expect(page.get_by_role("heading", name="Zucchiniloaf")).to_be_visible()


def test_stale_recipe_response_does_not_replace_the_current_one(
    page: Page, ui_url: str, api_url: str
):
    """Navigate A -> B -> back to A while B's request is still in flight: B
    landing last must not blank or replace A."""
    username, password, (first_id, second_id) = create_user_with_recipes(
        api_url, ["Applesauce", "Zucchiniloaf"], []
    )
    log_in(page, ui_url, username, password)

    page.goto(f"{ui_url}/recipes/{first_id}?randomQ=Zucchiniloaf")
    expect(page.get_by_role("heading", name="Applesauce")).to_be_visible()

    pending: list[Route] = []
    page.route(f"**/api/recipes/{second_id}", lambda route: pending.append(route))
    with page.expect_request(f"**/api/recipes/{second_id}"):
        page.get_by_role("button", name="Next Random").click()
    expect(page).to_have_url(re.compile(str(second_id)))
    assert len(pending) == 1

    page.go_back()
    expect(page.get_by_role("heading", name="Applesauce")).to_be_visible()

    with page.expect_response(f"**/api/recipes/{second_id}"):
        for route in pending:
            route.continue_()
    page.unroute(f"**/api/recipes/{second_id}")

    expect(page.get_by_role("heading", name="Applesauce")).to_be_visible()
    expect(page.get_by_text("Loading recipe...")).not_to_be_visible()


def test_switching_versions_hides_the_previous_version(
    page: Page, ui_url: str, api_url: str
):
    """Viewing another version of the same recipe must not leave the previous
    version on screen (with its revert/edit actions) while the new one loads."""
    username, password, (recipe_id,) = create_user_with_recipes(
        api_url, ["Plumjam"], []
    )
    with ApiClient(Configuration(host=api_url)) as client:
        token = (
            AuthApi(client)
            .login(LoginRequest(username=username, password=password))
            .token
        )
    authed_config = Configuration(host=api_url)
    authed_config.access_token = token
    with ApiClient(authed_config) as client:
        recipes_api = RecipesApi(client)
        current = recipes_api.get_recipe(recipe_id)
        recipes_api.update_recipe(
            recipe_id,
            UpdateRecipeRequest(
                title="Plumjelly",
                instructions=current.instructions,
                ingredients=current.ingredients,
                expected_version_id=current.version_id,
            ),
        )

    log_in(page, ui_url, username, password)
    page.goto(f"{ui_url}/recipes/{recipe_id}")
    expect(page.get_by_role("heading", name="Plumjelly")).to_be_visible()

    page.get_by_text("Version History").click()
    old_version = page.locator(".version-item").filter(
        has=page.locator(".version-item-title", has_text=re.compile("^Plumjam$"))
    )

    def is_version_fetch(url: str) -> bool:
        return f"/api/recipes/{recipe_id}?" in url and "version_id=" in url

    pending: list[Route] = []
    page.route(is_version_fetch, lambda route: pending.append(route))
    with page.expect_request(lambda request: is_version_fetch(request.url)):
        old_version.get_by_role("button", name="View").click()

    expect(page.get_by_text("Loading recipe...")).to_be_visible()
    expect(page.get_by_role("heading", name="Plumjelly")).not_to_be_visible()

    for route in pending:
        route.continue_()
    page.unroute(is_version_fetch)
    expect(page.get_by_role("heading", name="Plumjam")).to_be_visible()


def test_changing_weeks_shows_loading_until_the_new_week_arrives(
    page: Page, ui_url: str, api_url: str
):
    """Reloading the same week updates in place, but a different week must not
    render as an empty grid while its meal plans are still loading."""
    username, password = create_user_with_tags(api_url, [])
    log_in(page, ui_url, username, password)

    page.goto(f"{ui_url}/meal-plan")
    expect(page.locator(".meal-plan-grid")).to_be_visible()

    def is_meal_plan_list(url: str) -> bool:
        return "/api/meal-plans?" in url

    pending: list[Route] = []
    page.route(is_meal_plan_list, lambda route: pending.append(route))
    with page.expect_request(lambda request: is_meal_plan_list(request.url)):
        page.get_by_role("button", name="Next →").click()

    expect(page.get_by_text("Loading meal plans...")).to_be_visible()
    expect(page.locator(".meal-plan-grid")).not_to_be_visible()

    for route in pending:
        route.continue_()
    page.unroute(is_meal_plan_list)
    expect(page.locator(".meal-plan-grid")).to_be_visible()
    expect(page.get_by_text("Loading meal plans...")).not_to_be_visible()
