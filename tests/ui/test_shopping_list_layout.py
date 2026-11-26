"""Browser regressions for what a shopping-list row shows and its aisle menu."""

import re
import uuid

from playwright.sync_api import Page, expect

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, RecipesApi, SyncApi
from ramekin_client.models import (
    CreateRecipeRequest,
    OpKind,
    ShoppingListFields,
    ShoppingListOp,
    SignupRequest,
    SyncShoppingListRequest,
)

RECIPE = "Layout bean stew"


def test_row_shows_note_and_source_and_moves_aisles(
    page: Page, api_url: str, ui_url: str
):
    configuration = Configuration(host=api_url)
    with ApiClient(configuration) as client:
        signup = AuthApi(client).signup(
            SignupRequest(
                username=f"sllayout_{uuid.uuid4().hex[:12]}", password="testpass123"
            )
        )
    configuration.access_token = signup.token
    with ApiClient(configuration) as client:
        recipe = RecipesApi(client).create_recipe(
            CreateRecipeRequest(title=RECIPE, instructions="Stew.", ingredients=[])
        )
        SyncApi(client).sync_shopping_list(
            SyncShoppingListRequest(
                ops=[
                    ShoppingListOp(
                        op=OpKind.CREATE,
                        id=str(uuid.uuid4()),
                        fields=ShoppingListFields(
                            item="carrots",
                            amount="3",
                            note="the big ones",
                            source_recipe_id=recipe.id,
                            source_recipe_title=RECIPE,
                        ),
                    )
                ]
            )
        )
    page.goto(ui_url)
    page.evaluate("token => localStorage.setItem('token', token)", signup.token)
    page.goto(f"{ui_url}/shopping-list")

    row = page.locator(".shopping-item").filter(
        has=page.locator(".shopping-item-name", has_text=re.compile("^carrots$"))
    )
    expect(row.locator(".shopping-item-amount")).to_have_text("3")
    expect(row.locator(".shopping-item-note")).to_have_text("the big ones")
    expect(row.get_by_role("link", name=RECIPE)).to_have_attribute(
        "href", f"/recipes/{recipe.id}"
    )
    expect(page.locator(".shopping-summary")).to_have_text("1 to get")

    def group_of_carrots():
        return page.locator(".shopping-category-group").filter(has=row)

    expect(group_of_carrots().locator(".shopping-category-header")).to_contain_text(
        "Produce"
    )
    row.get_by_label("Aisle for carrots").select_option("Snacks")
    expect(group_of_carrots().locator(".shopping-category-header")).to_contain_text(
        "Snacks"
    )
