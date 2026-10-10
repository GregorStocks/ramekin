"""Browser regressions for the shared modal's keyboard and focus lifecycle."""

import re
import time
import uuid
from datetime import datetime, timezone

import pytest
from playwright.sync_api import Locator, Page, Route, expect

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, RecipesApi
from ramekin_client.models import CreateRecipeRequest, Ingredient, SignupRequest


@pytest.fixture
def modal_page(page: Page, api_url: str, ui_url: str) -> Page:
    configuration = Configuration(host=api_url)
    with ApiClient(configuration) as client:
        signup = AuthApi(client).signup(
            SignupRequest(
                username=f"modal_{uuid.uuid4().hex[:12]}", password="testpass123"
            )
        )
    configuration.access_token = signup.token
    with ApiClient(configuration) as client:
        recipe = RecipesApi(client).create_recipe(
            CreateRecipeRequest(
                title="Modal test soup",
                instructions="Simmer until tender.",
                ingredients=[
                    Ingredient(item="carrots", measurements=[]),
                    Ingredient(item="onions", measurements=[]),
                ],
            )
        )
    page.goto(ui_url)
    page.evaluate("token => localStorage.setItem('token', token)", signup.token)
    page.goto(f"{ui_url}/recipes/{recipe.id}")
    expect(page.locator(".ingredients-list")).to_be_visible()
    return page


def open_recipe_modal(page: Page, title: str) -> Locator:
    more = page.locator(".recipe-more-actions > summary")
    more.focus()
    more.press("Enter")
    action = page.get_by_role("button", name=title, exact=True)
    action.focus()
    action.press("Enter")
    dialog = page.get_by_role("dialog", name=title, exact=True)
    expect(dialog).to_be_visible()
    expect(dialog.get_by_role("heading", name=title, exact=True)).to_be_focused()
    expect(dialog).to_have_attribute("aria-modal", "true")
    expect(page.locator("body")).to_have_css("overflow", "hidden")
    return dialog


@pytest.mark.parametrize("title", ["Add to Shopping List", "Add to Meal Plan"])
def test_dialog_keyboard_containment_and_return_focus(modal_page: Page, title: str):
    page = modal_page
    dialog = open_recipe_modal(page, title)
    heading = dialog.get_by_role("heading", name=title, exact=True)
    first = (
        dialog.get_by_role("button", name="Deselect All")
        if title == "Add to Shopping List"
        else dialog.get_by_label("Date", exact=True)
    )
    last = dialog.get_by_role("button", name=re.compile(r"^Add(?: \d+ items)?$"))

    # Even programmatic focus cannot move into the inert background.
    page.get_by_role("link", name="Edit", exact=True, include_hidden=True).evaluate(
        "element => element.focus()"
    )
    expect(heading).to_be_focused()

    page.keyboard.press("Tab")
    expect(first).to_be_focused()
    if title == "Add to Meal Plan":
        # Native date inputs have multiple internal tab stops (month/day/year).
        # Moving backwards within the date must not wrap to the footer action.
        page.keyboard.press("Tab")
        expect(first).to_be_focused()
        page.keyboard.press("Shift+Tab")
        expect(first).to_be_focused()
    page.keyboard.press("Shift+Tab")
    expect(last).to_be_focused()
    page.keyboard.press("Tab")
    expect(first).to_be_focused()
    heading.focus()
    page.keyboard.press("Shift+Tab")
    expect(last).to_be_focused()

    page.keyboard.press("Escape")
    expect(dialog).not_to_be_visible()
    expect(page.locator(".recipe-more-actions > summary")).to_be_focused()
    expect(page.locator("body")).not_to_have_css("overflow", "hidden")

    # A fresh dialog must work after native cancellation and DOM disposal.
    reopened = open_recipe_modal(page, title)
    reopened.get_by_role("button", name="Cancel", exact=True).click()
    expect(reopened).not_to_be_visible()
    expect(page.locator(".recipe-more-actions > summary")).to_be_focused()


def test_dialog_skips_disabled_controls_and_backdrop_restores_scroll(modal_page: Page):
    page = modal_page
    page.evaluate("document.body.style.overflow = 'scroll'")
    dialog = open_recipe_modal(page, "Add to Shopping List")
    select_all = dialog.get_by_role("button", name="Deselect All")
    select_all.click()
    expect(dialog.get_by_role("button", name="Add 0 items")).to_be_disabled()
    page.keyboard.press("Shift+Tab")
    expect(dialog.get_by_role("button", name="Cancel")).to_be_focused()
    page.keyboard.press("Tab")
    expect(dialog.get_by_role("button", name="Select All")).to_be_focused()

    # Clicking content must not dismiss; clicking the empty backdrop must.
    dialog.get_by_role("heading").click()
    expect(dialog).to_be_visible()
    page.mouse.click(4, 4)
    expect(dialog).not_to_be_visible()
    expect(page.locator(".recipe-more-actions > summary")).to_be_focused()
    expect(page.locator("body")).to_have_css("overflow", "scroll")


def test_dialog_keeps_focus_when_async_action_removes_controls(modal_page: Page):
    page = modal_page
    dialog = open_recipe_modal(page, "Add to Shopping List")
    heading = dialog.get_by_role("heading")
    pending: list[Route] = []
    page.route("**/api/shopping-list", lambda route: pending.append(route))
    frozen_time = datetime(2026, 9, 9, tzinfo=timezone.utc)
    page.clock.install(time=frozen_time)
    page.clock.pause_at(frozen_time)

    dialog.get_by_role("button", name="Add 2 items").click()
    expect(dialog.get_by_role("button", name="Adding...")).to_be_disabled()
    expect(heading).to_be_focused()
    assert len(pending) == 1

    # Put focus on a control that the success message will remove.
    dialog.get_by_role("checkbox").first.focus()
    pending[0].continue_()
    expect(dialog).to_contain_text("Added 2 items to shopping list!")
    expect(dialog.get_by_role("button")).to_have_count(0)
    expect(heading).to_be_focused()
    page.keyboard.press("Tab")
    expect(heading).to_be_focused()
    page.keyboard.press("Shift+Tab")
    expect(heading).to_be_focused()

    page.clock.run_for(1500)
    expect(dialog).not_to_be_visible()
    expect(page.locator(".recipe-more-actions > summary")).to_be_focused()
    expect(page.locator("body")).not_to_have_css("overflow", "hidden")


def hold_shopping_list_posts(page: Page) -> list[Route]:
    pending: list[Route] = []

    def handle(route: Route) -> None:
        if route.request.method == "POST":
            pending.append(route)
        else:
            route.continue_()

    page.route("**/api/shopping-list", handle)
    return pending


def wait_for_synced_shopping_item(page: Page, item: str) -> None:
    # Poll from Python: page.clock is paused, so in-page timers won't tick.
    for _ in range(100):
        if page.evaluate(
            """item => Object.keys(localStorage).some(key =>
                key.startsWith("shoppingListSyncCache") &&
                localStorage.getItem(key).includes(item))""",
            item,
        ):
            return
        time.sleep(0.05)
    raise AssertionError(f"{item!r} never reached the shopping list sync cache")


def expect_fresh_shopping_dialog(dialog: Locator) -> None:
    expect(dialog).to_be_visible()
    expect(dialog).not_to_contain_text("Added 2 items to shopping list!")
    expect(dialog.get_by_role("button", name="Add 2 items")).to_be_enabled()


def test_shopping_success_timer_does_not_close_reopened_dialog(modal_page: Page):
    page = modal_page
    frozen_time = datetime(2026, 9, 9, tzinfo=timezone.utc)
    page.clock.install(time=frozen_time)
    page.clock.pause_at(frozen_time)
    dialog = open_recipe_modal(page, "Add to Shopping List")

    dialog.get_by_role("button", name="Add 2 items").click()
    expect(dialog).to_contain_text("Added 2 items to shopping list!")
    page.keyboard.press("Escape")
    expect(dialog).not_to_be_visible()

    reopened = open_recipe_modal(page, "Add to Shopping List")
    page.clock.run_for(1500)
    expect_fresh_shopping_dialog(reopened)


def test_shopping_response_after_dismissal_leaves_reopened_dialog_alone(
    modal_page: Page,
):
    page = modal_page
    pending = hold_shopping_list_posts(page)
    frozen_time = datetime(2026, 9, 9, tzinfo=timezone.utc)
    page.clock.install(time=frozen_time)
    page.clock.pause_at(frozen_time)
    dialog = open_recipe_modal(page, "Add to Shopping List")

    dialog.get_by_role("button", name="Add 2 items").click()
    expect(dialog.get_by_role("button", name="Adding...")).to_be_disabled()
    assert len(pending) == 1
    page.keyboard.press("Escape")
    expect(dialog).not_to_be_visible()

    reopened = open_recipe_modal(page, "Add to Shopping List")
    pending[0].continue_()
    # The stale submission still refreshes the sync cache; once that lands it
    # has reached the point where it used to show success and arm the close.
    wait_for_synced_shopping_item(page, "carrots")
    expect_fresh_shopping_dialog(reopened)
    page.clock.run_for(1500)
    expect_fresh_shopping_dialog(reopened)


def flush_page_tasks(page: Page) -> None:
    # MessageChannel tasks aren't faked by page.clock, so these let a resolved
    # fetch's handlers run while timers stay frozen.
    page.evaluate(
        """async () => {
            for (let i = 0; i < 10; i++) {
                await new Promise(resolve => {
                    const channel = new MessageChannel();
                    channel.port1.onmessage = resolve;
                    channel.port2.postMessage(null);
                });
            }
        }"""
    )


def expect_fresh_meal_plan_dialog(dialog: Locator) -> None:
    expect(dialog).to_be_visible()
    expect(dialog).not_to_contain_text("Added to meal plan!")
    expect(dialog.get_by_role("button", name="Add", exact=True)).to_be_enabled()


def test_meal_plan_success_timer_does_not_close_reopened_dialog(modal_page: Page):
    page = modal_page
    frozen_time = datetime(2026, 9, 9, tzinfo=timezone.utc)
    page.clock.install(time=frozen_time)
    page.clock.pause_at(frozen_time)
    dialog = open_recipe_modal(page, "Add to Meal Plan")

    dialog.get_by_role("button", name="Add", exact=True).click()
    expect(dialog).to_contain_text("Added to meal plan!")
    page.keyboard.press("Escape")
    expect(dialog).not_to_be_visible()

    reopened = open_recipe_modal(page, "Add to Meal Plan")
    page.clock.run_for(1500)
    expect_fresh_meal_plan_dialog(reopened)


def test_meal_plan_response_after_dismissal_leaves_reopened_dialog_alone(
    modal_page: Page,
):
    page = modal_page
    pending: list[Route] = []
    page.route(
        "**/api/meal-plans",
        lambda route: (
            pending.append(route)
            if route.request.method == "POST"
            else route.continue_()
        ),
    )
    frozen_time = datetime(2026, 9, 9, tzinfo=timezone.utc)
    page.clock.install(time=frozen_time)
    page.clock.pause_at(frozen_time)
    dialog = open_recipe_modal(page, "Add to Meal Plan")

    dialog.get_by_role("button", name="Add", exact=True).click()
    expect(dialog.get_by_role("button", name="Adding...")).to_be_disabled()
    assert len(pending) == 1
    page.keyboard.press("Escape")
    expect(dialog).not_to_be_visible()

    reopened = open_recipe_modal(page, "Add to Meal Plan")
    with page.expect_response("**/api/meal-plans") as response_info:
        pending[0].continue_()
    response_info.value.finished()
    flush_page_tasks(page)
    expect_fresh_meal_plan_dialog(reopened)
    page.clock.run_for(1500)
    expect_fresh_meal_plan_dialog(reopened)


def test_dialog_restores_direct_opener_and_cleans_up_on_navigation(
    logged_in_page: Page, ui_url: str
):
    page = logged_in_page
    page.goto(f"{ui_url}/settings")
    page.evaluate("document.body.style.overflow = 'scroll'")
    opener = page.get_by_role("button", name="Sign Out", exact=True)
    opener.click()
    dialog = page.get_by_role("dialog", name="Sign out of Ramekin?")
    expect(dialog.get_by_role("heading")).to_be_focused()
    page.keyboard.press("Escape")
    expect(opener).to_be_focused()
    expect(page.locator("body")).to_have_css("overflow", "scroll")

    opener.click()
    dialog.get_by_role("button", name="Sign Out", exact=True).click()
    expect(page).to_have_url(re.compile(r"/login$"))
    expect(page.get_by_role("dialog")).to_have_count(0)
    expect(page.locator("body")).to_have_css("overflow", "scroll")
    username = page.get_by_role("textbox", name="Username", exact=True)
    username.focus()
    expect(username).to_be_focused()
