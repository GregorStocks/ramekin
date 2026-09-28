"""
Tests for the recipe form's ingredient editor: section headings are rows in
the same list as ingredients, so renaming a section keeps focus, dragging an
ingredient past a heading moves it between sections, and headings can be
deleted explicitly.
"""

import uuid

from playwright.sync_api import Locator, Page, expect

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, RecipesApi
from ramekin_client.models import (
    CreateRecipeRequest,
    Ingredient,
    Measurement,
    SignupRequest,
)


def _row_for(page: Page, item: str) -> Locator:
    """The editor row whose ingredient input currently holds `item`."""
    entries = page.locator(".ingredient-editor .ingredient-entry")
    items = [
        entries.nth(i).get_by_role("textbox", name="Ingredient", exact=True)
        for i in range(entries.count())
    ]
    values = [field.input_value() for field in items]
    return entries.nth(values.index(item))


def _drag(page: Page, source: Locator, target: Locator) -> None:
    """Drag with intermediate pointer moves so the drag sensor activates."""
    src = source.bounding_box()
    dst = target.bounding_box()
    assert src is not None and dst is not None
    start_x = src["x"] + src["width"] / 2
    start_y = src["y"] + src["height"] / 2
    page.mouse.move(start_x, start_y)
    page.mouse.down()
    page.mouse.move(start_x, dst["y"] + dst["height"] / 2, steps=12)
    page.mouse.up()


def test_ingredient_editor_sections(api_url: str, ui_url: str, page: Page):
    username = f"sections_{uuid.uuid4().hex[:8]}"
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
        recipes_api = RecipesApi(client)
        recipe_id = recipes_api.create_recipe(
            CreateRecipeRequest(
                title="Sectioned Cake",
                instructions="Bake.",
                ingredients=[
                    Ingredient(item="flour", measurements=[Measurement(amount="2")]),
                    Ingredient(
                        item="milk",
                        measurements=[Measurement(amount="1")],
                        section="Batter",
                    ),
                    Ingredient(
                        item="eggs",
                        measurements=[Measurement(amount="3")],
                        section="Batter",
                    ),
                    Ingredient(
                        item="powdered sugar",
                        measurements=[Measurement(amount="1")],
                        section="Glaze",
                    ),
                ],
            )
        ).id

    page.goto(ui_url)
    page.wait_for_selector("input[type='text']")
    page.fill("input[type='text']", username)
    page.fill("input[type='password']", password)
    page.click("button[type='submit']")
    page.wait_for_selector(".recipe-card")
    page.goto(f"{ui_url.rstrip('/')}/recipes/{recipe_id}/edit")

    section_names = page.get_by_role("textbox", name="Section name")
    expect(section_names).to_have_count(2)

    # Typing into a section name must not re-create the input under the cursor.
    batter = section_names.first
    batter.click()
    page.keyboard.press("End")
    batter_handle = batter.element_handle()
    page.keyboard.type(" Mix")
    expect(section_names.first).to_have_value("Batter Mix")
    assert page.evaluate("(el) => document.activeElement === el", batter_handle)

    # Drag the unsectioned flour down past the Batter heading, after eggs.
    _drag(
        page,
        _row_for(page, "flour").locator(".drag-handle"),
        _row_for(page, "eggs"),
    )
    expect(page.locator(".ingredient-entry.in-section")).to_have_count(4)

    # Deleting the Glaze heading keeps its ingredient under the heading above.
    page.get_by_role("button", name="Delete section").nth(1).click()
    expect(section_names).to_have_count(1)

    page.get_by_role("button", name="+ Section").click()
    expect(section_names.nth(1)).to_be_focused()
    page.keyboard.type("Garnish")
    page.get_by_role("textbox", name="Ingredient", exact=True).last.fill("mint")

    page.get_by_role("button", name="Save Changes").click()
    page.wait_for_selector(".ingredients-list")

    with ApiClient(config) as client:
        saved = RecipesApi(client).get_recipe(recipe_id)
    assert [(i.item, i.section) for i in saved.ingredients] == [
        ("milk", "Batter Mix"),
        ("eggs", "Batter Mix"),
        ("flour", "Batter Mix"),
        ("powdered sugar", "Batter Mix"),
        ("mint", "Garnish"),
    ]
