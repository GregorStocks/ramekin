"""End-to-end UI test for the scrape status page.

Drives the full flow: from the new-recipe page, submit a URL, watch the
status page render its canonical steps, wait for completion, expand a step
to see the JSON output, and confirm the View Recipe button appears.
"""

from playwright.sync_api import Page, expect


def test_scrape_status_page_renders_steps(
    logged_in_page: Page, ui_url: str, fixture_base_url: str
):
    page = logged_in_page
    page.goto(f"{ui_url}/recipes/new")

    # Kick off a scrape by pasting a URL and clicking Import.
    page.get_by_placeholder("Paste recipe URL...").fill(
        f"{fixture_base_url}/seriouseats/rice_pilaf.html"
    )
    page.get_by_role("button", name="Import", exact=True).click()

    # Navigation to /scrape/:id.
    page.wait_for_url("**/scrape/*", timeout=10_000)

    # The canonical step list should render with the first several step names.
    step_list = page.locator(".step-list")
    expect(step_list).to_be_visible()
    for step_name in ["fetch_html", "extract_recipe", "save_recipe"]:
        expect(step_list.get_by_text(step_name, exact=True)).to_be_visible()

    # Wait for the status pill to reach terminal "completed" state.
    completed_pill = page.locator('.status-pill[data-status="completed"]')
    expect(completed_pill).to_be_visible(timeout=60_000)

    # Expand fetch_html by clicking its row (it's a <button class="step-row">).
    fetch_html_row = step_list.locator(".step-row", has_text="fetch_html").first
    fetch_html_row.click()

    # JSON output block renders inside .step-output > pre.
    output_pre = page.locator(".step-output pre").first
    expect(output_pre).to_be_visible()
    # Sanity check: it should contain some JSON structure.
    assert output_pre.text_content().strip().startswith(("{", "[", '"')), (
        "expected JSON output in fetch_html expansion"
    )

    # View Recipe button is visible on successful completion.
    expect(page.get_by_role("button", name="View Recipe →")).to_be_visible()
