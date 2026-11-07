"""Browser regressions for meal-plan action results: the reload that follows
an add or edit must not wipe the message saying why it didn't apply."""

import re
import uuid
from dataclasses import dataclass
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

TITLE = "Op message tomato soup"


@dataclass
class PlannedPage:
    page: Page
    configuration: Configuration
    meal_id: str


@pytest.fixture
def planned_page(page: Page, api_url: str, ui_url: str) -> PlannedPage:
    configuration = Configuration(host=api_url)
    with ApiClient(configuration) as client:
        signup = AuthApi(client).signup(
            SignupRequest(
                username=f"opmsg_{uuid.uuid4().hex[:12]}", password="testpass123"
            )
        )
    configuration.access_token = signup.token
    meal_id = str(uuid.uuid4())
    with ApiClient(configuration) as client:
        recipe = RecipesApi(client).create_recipe(
            CreateRecipeRequest(title=TITLE, instructions="Cook.", ingredients=[])
        )
        SyncApi(client).sync_meal_plans(
            SyncMealPlansRequest(
                ops=[
                    MealPlanOp(
                        op=OpKind.CREATE,
                        id=meal_id,
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
    page.goto(f"{ui_url}/meal-plan")
    expect(page.locator(".meal-card")).to_contain_text(TITLE)
    return PlannedPage(page, configuration, meal_id)


def test_adding_a_duplicate_meal_says_so(planned_page: PlannedPage):
    page = planned_page.page
    slot = page.locator(".meal-slot.day-today").filter(
        has=page.get_by_role("button", name=re.compile(r"^Add Dinner on "))
    )
    slot.get_by_role("button", name=re.compile(r"^Add Dinner on ")).click()
    dialog = page.get_by_role("dialog", name=re.compile(r"^Add Dinner - "))
    dialog.get_by_role("button", name=TITLE, exact=True).click()

    expect(dialog).not_to_be_visible()
    alert = page.get_by_role("alert")
    expect(alert).to_have_text(
        "This recipe is already scheduled for dinner on this date"
    )
    # Still there once the week has reloaded.
    expect(slot.locator(".meal-card")).to_have_count(1)
    expect(alert).to_be_visible()


def test_editing_a_meal_removed_elsewhere_says_so(planned_page: PlannedPage):
    page = planned_page.page
    with ApiClient(planned_page.configuration) as client:
        SyncApi(client).sync_meal_plans(
            SyncMealPlansRequest(
                ops=[MealPlanOp(op=OpKind.DELETE, id=planned_page.meal_id)]
            )
        )

    page.get_by_role("button", name=f"Edit {TITLE}").click()
    dialog = page.get_by_role("dialog", name="Edit Meal")
    dialog.get_by_label("Notes").fill("Double batch")
    dialog.get_by_role("button", name="Save", exact=True).click()

    expect(dialog).not_to_be_visible()
    alert = page.get_by_role("alert")
    expect(alert).to_have_text("This meal was already removed")
    # Still there once the week has reloaded without the meal.
    expect(page.locator(".meal-card")).to_have_count(0)
    expect(alert).to_be_visible()
