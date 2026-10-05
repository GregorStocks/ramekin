"""Browser regression for the meal-plan card's icon-only edit/remove buttons."""

import uuid
from datetime import date

import pytest
from playwright.sync_api import Page, expect

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, MealPlansApi, RecipesApi
from ramekin_client.models import (
    CreateMealPlanRequest,
    CreateRecipeRequest,
    MealType,
    SignupRequest,
)

TITLE = "Card actions miso soup"


@pytest.fixture
def meal_plan_page(page: Page, api_url: str, ui_url: str) -> Page:
    configuration = Configuration(host=api_url)
    with ApiClient(configuration) as client:
        signup = AuthApi(client).signup(
            SignupRequest(
                username=f"cardacts_{uuid.uuid4().hex[:12]}", password="testpass123"
            )
        )
    configuration.access_token = signup.token
    with ApiClient(configuration) as client:
        recipe = RecipesApi(client).create_recipe(
            CreateRecipeRequest(title=TITLE, instructions="Cook.", ingredients=[])
        )
        MealPlansApi(client).create_meal_plan(
            CreateMealPlanRequest(
                recipe_id=recipe.id, meal_date=date.today(), meal_type=MealType.DINNER
            )
        )
    page.goto(ui_url)
    page.evaluate("token => localStorage.setItem('token', token)", signup.token)
    page.goto(f"{ui_url}/meal-plan")
    return page


def test_card_buttons_name_the_meal_and_show_on_focus(meal_plan_page: Page):
    page = meal_plan_page
    card = page.locator(".meal-card").filter(has_text=TITLE)
    edit = card.get_by_role("button", name=f"Edit {TITLE}", exact=True)
    remove = card.get_by_role("button", name=f"Remove {TITLE}", exact=True)
    expect(edit).to_be_attached()
    expect(remove).to_be_attached()

    # Hidden until hover; keyboard focus must reveal them too.
    expect(edit).to_have_css("opacity", "0")
    edit.focus()
    expect(edit).to_have_css("opacity", "1")
    expect(remove).to_have_css("opacity", "1")
