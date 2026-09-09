"""Exercise text creation through the real API and deterministic AI fixture."""

from playwright.sync_api import expect


RECIPE_TEXT = """Text Pancakes
1 cup all-purpose flour
8 oz butter
1 cup mystery powder
Mix ingredients. Cook in a pan.
"""


def test_text_review_correct_and_save(logged_in_page, ui_url):
    page = logged_in_page
    page.goto(f"{ui_url}/recipes/new")
    page.get_by_label("Recipe text", exact=True).fill(RECIPE_TEXT)
    page.get_by_role("button", name="Review recipe", exact=True).click()
    expect(page.get_by_role("heading", name="Review recipe")).to_be_visible(
        timeout=30000
    )
    expect(
        page.get_by_text("1 cup (125 g) all-purpose flour", exact=True)
    ).to_be_visible()
    page.get_by_label("Title", exact=False).fill("Reviewed text pancakes")
    page.get_by_label("Ingredients", exact=True).fill("2 cups all-purpose flour")
    page.get_by_role("button", name="Save recipe", exact=True).click()
    expect(
        page.get_by_role("heading", name="Reviewed text pancakes", exact=True)
    ).to_be_visible(timeout=30000)
    expect(page.get_by_text("250", exact=False).first).to_be_visible()


def test_failed_processing_preserves_text(logged_in_page, ui_url):
    page = logged_in_page
    page.goto(f"{ui_url}/recipes/new")
    text = "TEXT_EXTRACTION_FAILURE\n" + RECIPE_TEXT
    page.get_by_label("Recipe text", exact=True).fill(text)
    page.get_by_role("button", name="Review recipe", exact=True).click()
    expect(page.get_by_role("alert")).to_be_visible(timeout=30000)
    expect(page.get_by_label("Recipe text", exact=True)).to_have_value(text)


def test_failed_save_preserves_review_edits(logged_in_page, ui_url):
    page = logged_in_page
    page.goto(f"{ui_url}/recipes/new")
    page.get_by_label("Recipe text", exact=True).fill(RECIPE_TEXT)
    page.get_by_role("button", name="Review recipe", exact=True).click()
    expect(page.get_by_role("heading", name="Review recipe")).to_be_visible(
        timeout=30000
    )
    page.get_by_label("Title", exact=False).fill("Keep my corrections")
    page.get_by_label("Ingredients", exact=True).fill("3 cups all-purpose flour")
    page.route(
        "**/api/recipes",
        lambda route: route.fulfill(
            status=503,
            content_type="application/json",
            body='{"code":"service_unavailable","error":"Please retry saving."}',
        ),
    )
    page.get_by_role("button", name="Save recipe", exact=True).click()
    expect(page.get_by_text("Please retry saving.")).to_be_visible()
    expect(page.get_by_label("Title", exact=False)).to_have_value("Keep my corrections")
    expect(page.get_by_label("Ingredients", exact=True)).to_have_value(
        "3 cups all-purpose flour"
    )
