"""Rendered regressions for recipe actions, responsive controls, and focus."""

import re
import uuid

import pytest
from playwright.sync_api import Locator, Page, expect

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi
from ramekin_client.models import SignupRequest


def contrast_ratio(element: Locator) -> float:
    """Measure opaque button text against its actual computed background."""
    colors = element.evaluate(
        "el => { const s = getComputedStyle(el); return [s.color, s.backgroundColor]; }"
    )
    luminances = []
    for color in colors:
        channels = [float(value) for value in re.findall(r"[\d.]+", color)]
        assert len(channels) == 3 or channels[3] == 1, color
        linear = [
            value / 255 / 12.92
            if value / 255 <= 0.04045
            else ((value / 255 + 0.055) / 1.055) ** 2.4
            for value in channels[:3]
        ]
        luminances.append(sum(a * b for a, b in zip(linear, (0.2126, 0.7152, 0.0722))))
    return (max(luminances) + 0.05) / (min(luminances) + 0.05)


def assert_no_horizontal_overflow(page: Page):
    assert page.evaluate("document.documentElement.scrollWidth <= window.innerWidth")


@pytest.mark.parametrize("width", [320, 390, 768, 1920, 2560])
@pytest.mark.parametrize("color_scheme", ["light", "dark"])
def test_cookbook_actions_are_readable(
    logged_in_page: Page, width: int, color_scheme: str
):
    page = logged_in_page
    page.set_viewport_size({"width": width, "height": 1080})
    page.emulate_media(color_scheme=color_scheme, reduced_motion="reduce")

    density = page.get_by_role("group", name="Recipe density")
    for label in ("Cards", "Compact", "List"):
        button = density.get_by_role("button", name=label, exact=True)
        expect(button).to_be_in_viewport()
        assert button.evaluate("el => el.scrollWidth <= el.clientWidth")
    assert_no_horizontal_overflow(page)

    if width <= 960:
        page.get_by_role("button", name="Open navigation menu").click()
    new_recipe = page.get_by_role("link", name="+ New Recipe")
    expect(new_recipe).to_be_visible()
    assert contrast_ratio(new_recipe) >= 4.5
    new_recipe.hover()
    # Finish the hover transition before measuring its colors.
    new_recipe.evaluate("el => el.getAnimations().forEach(a => a.finish())")
    assert contrast_ratio(new_recipe) >= 4.5


@pytest.mark.parametrize("width", [320, 390, 768, 1920])
def test_detail_actions_align_and_work_with_keyboard(logged_in_page: Page, width: int):
    page = logged_in_page
    page.set_viewport_size({"width": width, "height": 1080})
    page.locator(".recipe-card").first.click()
    expect(page.locator(".instructions")).to_be_visible()
    edit = page.get_by_role("link", name="Edit", exact=True)
    more = page.locator(".recipe-more-actions > summary")
    edit_box = edit.bounding_box()
    more_box = more.bounding_box()
    assert edit_box is not None and more_box is not None
    assert edit_box["height"] >= 44
    assert abs(edit_box["y"] - more_box["y"]) <= 1
    assert abs(edit_box["height"] - more_box["height"]) <= 1
    assert contrast_ratio(edit) >= 4.5
    assert_no_horizontal_overflow(page)

    more.focus()
    more.press("Enter")
    shopping = page.get_by_role("button", name="Add to Shopping List", exact=True)
    expect(shopping).to_be_visible()
    more.press("Tab")
    expect(shopping).to_be_focused()
    shopping.press("Escape")
    expect(shopping).not_to_be_visible()
    expect(more).to_be_focused()
    expect(more).to_have_css("outline-style", "solid")
    more.press("Enter")
    shopping.click()
    expect(page.locator(".modal-content")).to_be_visible()
    page.keyboard.press("Escape")
    expect(page.locator(".modal-content")).not_to_be_visible()


@pytest.mark.parametrize("width", [320, 390, 768, 1920])
def test_recipe_editor_controls_are_accessible(logged_in_page: Page, width: int):
    page = logged_in_page
    page.set_viewport_size({"width": width, "height": 1080})
    page.locator(".recipe-card").first.click()
    page.get_by_role("link", name="Edit", exact=True).click()
    ingredient = page.get_by_role("textbox", name="Ingredient", exact=True).first
    expect(ingredient).to_be_visible()
    ingredient.focus()
    page.keyboard.press("Tab")
    note = page.get_by_role("textbox", name="Ingredient note").first
    expect(note).to_be_focused()
    expect(note).to_have_css("outline-style", "solid")
    remove = page.get_by_role("button", name="Remove ingredient", exact=True).first
    box = remove.bounding_box()
    assert box is not None and box["width"] >= 44 and box["height"] >= 44
    assert_no_horizontal_overflow(page)

    photo = page.get_by_role("button", name="+ Add Photo")
    photo.focus()
    with page.expect_file_chooser():
        photo.press("Enter")

    cancel = page.locator(".form-actions").get_by_role("link", name="Cancel")
    save = page.get_by_role("button", name="Save Changes")
    cancel_box = cancel.bounding_box()
    save_box = save.bounding_box()
    assert cancel_box is not None and save_box is not None
    assert abs(cancel_box["height"] - save_box["height"]) <= 1


@pytest.mark.parametrize("width", [390, 1920])
def test_create_and_edit_recipe_with_responsive_form(
    page: Page, ui_url: str, api_url: str, width: int
):
    with ApiClient(Configuration(host=api_url)) as client:
        signup = AuthApi(client).signup(
            SignupRequest(
                username=f"design_{uuid.uuid4().hex[:12]}", password="testpass123"
            )
        )
    page.set_viewport_size({"width": width, "height": 1080})
    page.goto(ui_url)
    page.evaluate("token => localStorage.setItem('token', token)", signup.token)
    page.goto(f"{ui_url}/recipes/new")
    page.get_by_label("Title *", exact=True).fill("Design test soup")
    page.get_by_label("Instructions *", exact=True).fill("Simmer until tender.")
    page.get_by_role("textbox", name="Ingredient", exact=True).first.fill("carrots")
    page.get_by_role("textbox", name="Amount", exact=True).first.fill("2")
    page.get_by_role("textbox", name="Unit", exact=True).first.fill("cups")
    assert_no_horizontal_overflow(page)
    page.get_by_role("button", name="Create Recipe", exact=True).click()
    expect(page.locator(".recipe-header-compact h2")).to_have_text("Design test soup")
    expect(page.locator(".ingredients-list")).to_contain_text("carrots")

    page.get_by_role("link", name="Edit", exact=True).click()
    page.get_by_label("Title *", exact=True).fill("Carrot soup")
    page.get_by_role("button", name="Save Changes", exact=True).click()
    expect(page.locator(".recipe-header-compact h2")).to_have_text("Carrot soup")
    expect(page.locator(".instructions")).to_have_text("Simmer until tender.")
