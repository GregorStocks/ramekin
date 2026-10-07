"""End-to-end UI test for the scrape status page.

Drives the full flow: from the new-recipe page, submit a URL, watch the
status page render its canonical steps, wait for completion, expand a step
to see the JSON output, and confirm the View Recipe button appears.
"""

import json
import os
import uuid

import requests
from playwright.sync_api import Page, expect

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi
from ramekin_client.models import SignupRequest


def hold_description(marker: str, hold: bool) -> None:
    """Hold the mock AI's description answer for prompts containing marker."""
    port = os.environ["MOCK_OPENROUTER_PORT"]
    requests.get(
        f"http://localhost:{port}/test/description-hold",
        params={"marker": marker, "hold": str(hold).lower()},
        timeout=10,
    ).raise_for_status()


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


def test_recipe_opens_while_enrichment_runs(page: Page, ui_url: str, api_url: str):
    """The status page offers the recipe as soon as it is saved; the recipe
    page says enrichment is running and refreshes when it lands."""
    username = f"ui_enrich_{uuid.uuid4().hex[:8]}"
    password = "testpass123"
    with ApiClient(Configuration(host=api_url)) as client:
        token = (
            AuthApi(client)
            .signup(SignupRequest(username=username, password=password))
            .token
        )

    # Letters only, so the marker survives title normalization intact.
    marker = "zq" + uuid.uuid4().hex[:8].translate(
        str.maketrans("0123456789", "ghijklmnop")
    )
    title = f"Enriching {marker} soup"
    recipe = {
        "@context": "https://schema.org",
        "@type": "Recipe",
        "name": title,
        "recipeIngredient": ["1 tsp salt", "2 cups water"],
        "recipeInstructions": [{"@type": "HowToStep", "text": "Stir and serve."}],
    }
    html = (
        "<html><head><script type='application/ld+json'>"
        f"{json.dumps(recipe)}</script></head><body></body></html>"
    )

    hold_description(marker, True)
    try:
        response = requests.post(
            f"{api_url}/api/scrape/capture",
            json={
                "html": html,
                "source_url": f"https://www.seriouseats.com/{uuid.uuid4().hex}",
            },
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )
        response.raise_for_status()
        job_id = response.json()["id"]

        page.goto(ui_url)
        page.fill("input[type='text']", username)
        page.fill("input[type='password']", password)
        page.click("button[type='submit']")
        expect(page.locator("input[type='password']")).to_have_count(0)

        page.goto(f"{ui_url}/scrape/{job_id}")
        expect(page.locator('.status-pill[data-status="enriching"]')).to_be_visible(
            timeout=15_000
        )
        page.get_by_role("button", name="View Recipe →").click()

        page.wait_for_url("**/recipes/**")
        expect(page.get_by_role("heading", name=title)).to_be_visible()
        expect(page.locator(".enrichment-notice")).to_have_text(
            "Adding an AI title, description, and tags…"
        )
    finally:
        hold_description(marker, False)

    expect(page.get_by_text("A delicious test recipe.")).to_be_visible(timeout=15_000)
    expect(page.locator(".enrichment-notice")).to_have_count(0)
    assert "scrapeJob" not in page.url
