#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "playwright",
# ]
# ///
"""
Take a screenshot of the app as the test user.

Usage:
    uv run scripts/screenshot.py [--url URL] [--output PATH] [--page PAGE]

Pages:
    cookbook (default) - The main cookbook/recipe list page
    login - The login page (before auth)
    new - The create recipe page
    recipe - View the alphabetically first recipe by title

Requires the dev server to be running and seed data to exist.
"""

import argparse
import os

from playwright.sync_api import sync_playwright

# Default test user credentials (from seed command)
TEST_USERNAME = "t"
TEST_PASSWORD = "t"


def take_screenshot(
    base_url: str = "http://localhost:5173",
    output_path: str = "logs/screenshot.png",
    page_name: str = "cookbook",
    width: int = 1280,
    height: int = 800,
):
    """Take a screenshot of the specified page."""

    # Ensure output directory exists
    os.makedirs(os.path.dirname(output_path), exist_ok=True)

    with sync_playwright() as p:
        # Use system Chrome to avoid macOS permission issues with bundled Chromium
        browser = p.chromium.launch(headless=True, channel="chrome")
        context = browser.new_context(viewport={"width": width, "height": height})
        page = context.new_page()

        if page_name == "login":
            # Just show the login page without logging in
            page.goto(base_url)
            page.wait_for_load_state("networkidle")
        else:
            # Log in first
            page.goto(base_url)
            page.wait_for_load_state("networkidle")

            # Fill in login form
            page.fill('input[type="text"]', TEST_USERNAME)
            page.fill('input[type="password"]', TEST_PASSWORD)
            page.click('button[type="submit"]')

            # Wait for navigation to complete
            page.wait_for_load_state("networkidle")

            # Small delay to ensure any animations complete
            page.wait_for_timeout(500)

            # Navigate to the requested page
            if page_name == "cookbook":
                # Already on cookbook after login
                pass
            elif page_name == "new":
                page.goto(f"{base_url}/recipes/new")
                page.wait_for_load_state("networkidle")
            elif page_name == "recipe":
                # Find the alphabetically first recipe by title and click it
                cards = page.locator(".recipe-card")
                count = cards.count()
                if count == 0:
                    print("Warning: No recipes found")
                else:
                    # Get all recipe titles and find the alphabetically first one
                    titles_with_index = []
                    for i in range(count):
                        title = cards.nth(i).locator("h3").inner_text()
                        titles_with_index.append((title, i))
                    # Sort alphabetically by title
                    titles_with_index.sort(key=lambda x: x[0].lower())
                    first_title, first_index = titles_with_index[0]
                    print(f"Clicking recipe: {first_title}")
                    cards.nth(first_index).click()
                    page.wait_for_load_state("networkidle")
            elif page_name.startswith("recipe:"):
                # View a specific recipe by index (0-based)
                recipe_index = int(page_name.split(":")[1])
                # Click on the nth recipe card
                cards = page.locator(".recipe-card")
                if cards.count() > recipe_index:
                    cards.nth(recipe_index).click()
                    page.wait_for_load_state("networkidle")
                else:
                    print(f"Warning: Only {cards.count()} recipes found")

        # Take the screenshot
        page.screenshot(path=output_path)
        print(f"Screenshot saved to {output_path}")

        browser.close()


def main():
    parser = argparse.ArgumentParser(description="Take app screenshots")
    parser.add_argument(
        "--url",
        default=os.environ.get("APP_URL", "http://localhost:5173"),
        help="App URL (default: http://localhost:5173)",
    )
    parser.add_argument(
        "--output",
        "-o",
        default="logs/screenshot.png",
        help="Output path (default: logs/screenshot.png)",
    )
    parser.add_argument(
        "--page",
        "-p",
        default="cookbook",
        choices=["login", "cookbook", "new", "recipe"],
        help="Page to screenshot (default: cookbook)",
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
        default=800,
        help="Viewport height (default: 800)",
    )
    args = parser.parse_args()

    take_screenshot(
        base_url=args.url,
        output_path=args.output,
        page_name=args.page,
        width=args.width,
        height=args.height,
    )


if __name__ == "__main__":
    main()
