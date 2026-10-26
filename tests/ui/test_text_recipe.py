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
    flour = page.locator(".ingredients-list li").first
    expect(flour.locator(".amount")).to_have_text("~125")
    expect(flour.locator(".alt-measurement")).to_contain_text("(1 cup")


def test_failed_text_import_can_be_retried(logged_in_page, ui_url):
    page = logged_in_page
    page.goto(f"{ui_url}/recipes/new")
    page.get_by_label("Recipe text", exact=True).fill(
        "TEXT_EXTRACTION_FAILURE\n" + RECIPE_TEXT
    )
    page.get_by_role("button", name="Import recipe", exact=True).click()
    retry = page.get_by_role("button", name="Retry", exact=True)
    with page.expect_response("**/retry") as response:
        retry.click(timeout=30000)
    assert response.value.ok
    # The fixture fails this text every time, so the retry fails again.
    expect(page.locator(".error-banner")).to_contain_text(
        "at extract_recipe", timeout=30000
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
    messages = []

    # Answer the confirm from a handler: the click doesn't return while it's
    # open. Handlers only run while Playwright waits, so wait for it to.
    def click_and_answer(accept):
        count = len(messages)

        def handle(dialog):
            messages.append(dialog.message)
            dialog.accept() if accept else dialog.dismiss()

        page.once("dialog", handle)
        home.click()
        for _ in range(100):
            if len(messages) > count:
                break
            page.wait_for_timeout(50)
        assert len(messages) == count + 1, "no confirm before leaving"
        assert "unsaved changes" in messages[-1]

    click_and_answer(False)
    expect(page.get_by_label("Recipe text", exact=True)).to_have_value(RECIPE_TEXT)

    click_and_answer(True)
    expect(page).not_to_have_url(f"{ui_url}/recipes/new")
