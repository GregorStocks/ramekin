import os

import pytest
from playwright.sync_api import Page


@pytest.fixture
def ui_url():
    url = os.environ.get("UI_BASE_URL")
    if not url:
        raise ValueError("UI_BASE_URL environment variable required")
    return url


@pytest.fixture
def api_url():
    url = os.environ.get("API_BASE_URL")
    if not url:
        raise ValueError("API_BASE_URL environment variable required")
    return url


@pytest.fixture
def logged_in_page(page: Page, ui_url: str) -> Page:
    """Navigate to the UI and log in as test user."""
    page.goto(ui_url)

    # Wait for login form
    page.wait_for_selector("input[type='text']")

    # Log in with test credentials (created by seed)
    page.fill("input[type='text']", "t")
    page.fill("input[type='password']", "t")
    page.click("button[type='submit']")

    # Wait for redirect to cookbook page
    page.wait_for_selector(".recipe-card")

    return page
