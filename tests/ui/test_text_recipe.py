"""Exercise text import through the real API and deterministic AI fixture."""

from playwright.sync_api import expect


RECIPE_TEXT = """Text Pancakes
1 cup all-purpose flour
8 oz butter
1 cup unknowable powder
Mix ingredients. Cook in a pan.
"""


def test_text_import_follows_the_job_to_the_recipe(logged_in_page, ui_url):
    page = logged_in_page
    page.goto(f"{ui_url}/recipes/new")
    page.get_by_label("Recipe text", exact=True).fill(RECIPE_TEXT)
    page.get_by_role("button", name="Import recipe", exact=True).click()
    expect(page.get_by_text('extracted "Text Pancakes"')).to_be_visible(timeout=30000)
    page.get_by_role("button", name="View Recipe →").click()
    expect(page.get_by_role("heading", name="Text Pancakes", exact=True)).to_be_visible(
        timeout=30000
    )
    # The saved recipe shows the grams computed when it is read.
    expect(page.locator(".ingredients-list .alt-measurement").first).to_have_text(
        "(~125 g)"
    )


def test_failed_start_preserves_text(logged_in_page, ui_url):
    page = logged_in_page
    page.goto(f"{ui_url}/recipes/new")
    page.route(
        "**/api/import/text",
        lambda route: route.fulfill(
            status=503,
            content_type="application/json",
            body='{"code":"service_unavailable","error":"Please retry."}',
        ),
    )
    page.get_by_label("Recipe text", exact=True).fill(RECIPE_TEXT)
    page.get_by_role("button", name="Import recipe", exact=True).click()
    expect(page.get_by_role("alert")).to_have_text("Please retry.")
    expect(page.get_by_label("Recipe text", exact=True)).to_have_value(RECIPE_TEXT)


def test_leaving_with_pasted_text_asks_first(logged_in_page, ui_url):
    page = logged_in_page
    page.goto(f"{ui_url}/recipes/new")
    page.get_by_label("Recipe text", exact=True).fill(RECIPE_TEXT)
    home = page.locator("a.app-title")

    with page.expect_event("dialog") as dialog_info:
        home.click()
    assert "unsaved changes" in dialog_info.value.message
    dialog_info.value.dismiss()
    expect(page.get_by_label("Recipe text", exact=True)).to_have_value(RECIPE_TEXT)

    with page.expect_event("dialog") as dialog_info:
        home.click()
    dialog_info.value.accept()
    expect(page).not_to_have_url(f"{ui_url}/recipes/new")
