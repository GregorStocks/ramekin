"""Browser regressions for the meal plan's wide grid and narrow day list."""

import uuid
from datetime import date

import pytest
from playwright.sync_api import Page, expect

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, RecipesApi, SyncApi
from ramekin_client.models import (
    CreateRecipeRequest,
    MealPlanFields,
    MealPlanOp,
    MealType,
    OpKind,
    SignupRequest,
    SyncMealPlansRequest,
)

TITLE = "Layout lentil soup"


@pytest.fixture
def signed_in_with_dinner_today(page: Page, api_url: str, ui_url: str) -> Page:
    configuration = Configuration(host=api_url)
    with ApiClient(configuration) as client:
        signup = AuthApi(client).signup(
            SignupRequest(
                username=f"mplayout_{uuid.uuid4().hex[:12]}", password="testpass123"
            )
        )
    configuration.access_token = signup.token
    with ApiClient(configuration) as client:
        recipe = RecipesApi(client).create_recipe(
            CreateRecipeRequest(title=TITLE, instructions="Simmer.", ingredients=[])
        )
        SyncApi(client).sync_meal_plans(
            SyncMealPlansRequest(
                ops=[
                    MealPlanOp(
                        op=OpKind.CREATE,
                        id=str(uuid.uuid4()),
                        fields=MealPlanFields(
                            recipe_id=recipe.id,
                            meal_date=date.today(),
                            meal_type=MealType.DINNER,
                        ),
                    )
                ]
            )
        )
    page.goto(ui_url)
    page.evaluate("token => localStorage.setItem('token', token)", signup.token)
    return page


def test_narrow_week_is_a_day_list_scrolled_to_today(
    signed_in_with_dinner_today: Page, ui_url: str
):
    page = signed_in_with_dinner_today
    page.set_viewport_size({"width": 390, "height": 844})
    page.goto(f"{ui_url}/meal-plan")

    today = page.locator(".meal-day.day-today")
    expect(today.locator(".meal-card")).to_contain_text(TITLE)
    expect(today.locator(".day-header")).to_be_in_viewport()

    # Each slot names its meal on the same row as its add button.
    dinner = today.locator(".meal-slot").filter(has_text=TITLE)
    label = dinner.locator(".meal-slot-label")
    add = dinner.get_by_role("button", name="Add Dinner on ")
    expect(label).to_have_text("Dinner")
    expect(add).to_be_visible()
    label_box = label.bounding_box()
    add_box = add.bounding_box()
    assert label_box and add_box
    assert add_box["x"] > label_box["x"] + label_box["width"]
    assert (
        abs(
            (add_box["y"] + add_box["height"] / 2)
            - (label_box["y"] + label_box["height"] / 2)
        )
        < 4
    )
    expect(page.locator(".meal-row-label").first).to_be_hidden()


def test_wide_week_labels_rows_once(signed_in_with_dinner_today: Page, ui_url: str):
    page = signed_in_with_dinner_today
    page.set_viewport_size({"width": 1440, "height": 900})
    page.goto(f"{ui_url}/meal-plan")

    expect(page.locator(".meal-card")).to_contain_text(TITLE)
    expect(page.locator(".meal-row-label")).to_have_text(
        ["Breakfast", "Lunch", "Dinner", "Snack"]
    )
    expect(page.locator(".meal-slot-label").first).to_be_hidden()
    expect(page.locator(".meal-day")).to_have_count(7)
