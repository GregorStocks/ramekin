#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "playwright",
# ]
# ///
"""
Take screenshots of the app as the test user.

Usage:
    uv run scripts/screenshot.py [--url URL] [--output-dir DIR]

Takes three screenshots:
    - cookbook.png: The main cookbook/recipe list page
    - recipe.png: The alphabetically first recipe by title
    - edit.png: The edit page for that recipe

Requires the dev server to be running and seed data to exist.
"""

import argparse
import os

from playwright.sync_api import sync_playwright

# Default test user credentials (from seed command)
TEST_USERNAME = "t"
TEST_PASSWORD = "t"


def take_screenshots(
    base_url: str = "http://localhost:5173",
    output_dir: str = "logs",
    width: int = 1280,
    height: int = 800,
):
    """Take screenshots of cookbook, recipe, and edit pages."""
    os.makedirs(output_dir, exist_ok=True)

    with sync_playwright() as p:
        # Use bundled Chromium (works across platforms)
        # Note: Requires playwright browsers to be installed first
        # Run: uv run --with playwright -- playwright install chromium
        browser = p.chromium.launch(headless=True)
        context = browser.new_context(viewport={"width": width, "height": height})
        page = context.new_page()

        # Log in
        page.goto(base_url)
        page.wait_for_load_state("networkidle")
        page.fill('input[type="text"]', TEST_USERNAME)
        page.fill('input[type="password"]', TEST_PASSWORD)
        page.click('button[type="submit"]')
        page.wait_for_load_state("networkidle")
        page.wait_for_timeout(500)

        # Screenshot 1: Cookbook page
        cookbook_path = os.path.join(output_dir, "cookbook.png")
        page.screenshot(path=cookbook_path)
        print(f"Screenshot saved to {cookbook_path}")

        # Find the alphabetically first recipe by title
        cards = page.locator(".recipe-card")
        count = cards.count()
        if count == 0:
            print("Warning: No recipes found, skipping recipe and edit screenshots")
            browser.close()
            return

        titles_with_index = []
        for i in range(count):
            title = cards.nth(i).locator("h3").inner_text()
            titles_with_index.append((title, i))
        titles_with_index.sort(key=lambda x: x[0].lower())
        first_title, first_index = titles_with_index[0]
        print(f"Selected recipe: {first_title}")

        # Screenshot 2: Recipe page
        cards.nth(first_index).click()
        page.wait_for_load_state("networkidle")
        page.wait_for_timeout(300)
        recipe_path = os.path.join(output_dir, "recipe.png")
        page.screenshot(path=recipe_path)
        print(f"Screenshot saved to {recipe_path}")

        # Screenshot 3: Edit page
        page.click("a:has-text('Edit')")
        page.wait_for_load_state("networkidle")
        page.wait_for_timeout(300)
        edit_path = os.path.join(output_dir, "edit.png")
        page.screenshot(path=edit_path)
        print(f"Screenshot saved to {edit_path}")

        browser.close()


def main():
    parser = argparse.ArgumentParser(description="Take app screenshots")
    parser.add_argument(
        "--url",
        default=os.environ.get("APP_URL", "http://localhost:5173"),
        help="App URL (default: http://localhost:5173)",
    )
    parser.add_argument(
        "--output-dir",
        "-o",
        default="logs",
        help="Output directory (default: logs)",
    )
    parser.add_argument(
        "--width",
        type=int,
        default=1280,
        help="Viewport width (default: 1280)",
    )
    parser.add_argument(
        "--height",
        type=int,
        default=1200,
        help="Viewport height (default: 1200)",
    )
    args = parser.parse_args()

    take_screenshots(
        base_url=args.url,
        output_dir=args.output_dir,
        width=args.width,
        height=args.height,
    )


if __name__ == "__main__":
    main()
