"""Browser regressions for the shared modal's keyboard and focus lifecycle."""

import re
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
