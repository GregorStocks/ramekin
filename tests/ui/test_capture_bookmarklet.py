"""End-to-end UI tests for the save-recipe bookmarklet (capture.js).

Simulates the bookmarklet by injecting capture.js (served from the UI origin)
into a fixture recipe page, exactly like the real shim does, and asserts:
  - the happy path logs each step and reaches "Recipe saved!";
  - a hung capture POST surfaces the per-request timeout instead of spinning;
  - a job that never leaves "pending" trips the overall stall watchdog.

The capture.js timeouts are overridden via query params so the watchdog tests
finish in ~1-2s instead of the 30-60s production defaults.
"""

from playwright.sync_api import Page, expect


def _get_token(page: Page) -> str:
    token = page.evaluate("() => localStorage.getItem('token')")
    assert token, "expected an auth token in localStorage after login"
    return token


def _inject_bookmarklet(
    page: Page, ui_url: str, api_url: str, token: str, extra: str = ""
) -> None:
    """Append capture.js to the page the way the bookmarklet shim does."""
    page.evaluate(
        """({uiUrl, apiUrl, token, extra}) => {
            const s = document.createElement('script');
            s.src = uiUrl + '/capture.js?token=' + encodeURIComponent(token)
                + '&api=' + encodeURIComponent(apiUrl)
                + '&external=' + encodeURIComponent(uiUrl)
                + extra
                + '&t=' + Date.now();
            document.body.appendChild(s);
        }""",
        {"uiUrl": ui_url, "apiUrl": api_url, "token": token, "extra": extra},
    )


def test_capture_happy_path_logs_and_saves(
    logged_in_page: Page, ui_url: str, api_url: str, fixture_base_url: str
):
    page = logged_in_page
    token = _get_token(page)

    logs: list[str] = []
    page.on("console", lambda msg: logs.append(msg.text))

    page.goto(f"{fixture_base_url}/seriouseats/rice_pilaf.html")
    _inject_bookmarklet(page, ui_url, api_url, token)

    # Overlay reaches the terminal success state.
    message = page.locator("#ramekin-message")
    expect(message).to_have_text("Recipe saved!", timeout=60_000)
    expect(page.get_by_text("View Recipe", exact=True)).to_be_visible()

    # Console captured the full step trace.
    joined = "\n".join(logs)
    assert "[Ramekin +" in joined, joined
    for needle in [
        "captured HTML",
        "POST /api/scrape/capture",
        "capture ok, job=",
        "poll #1",
        "completed, recipe=",
    ]:
        assert needle in joined, f"missing {needle!r} in console:\n{joined}"


def test_capture_post_timeout_surfaces(
    logged_in_page: Page, ui_url: str, api_url: str, fixture_base_url: str
):
    page = logged_in_page
    token = _get_token(page)

    logs: list[str] = []
    page.on("console", lambda msg: logs.append(msg.text))

    # Hold the capture POST open forever so the client-side timeout must fire.
    def handle(route):
        if route.request.method == "POST":
            return  # never resolve -> request hangs
        route.continue_()

    page.goto(f"{fixture_base_url}/seriouseats/rice_pilaf.html")
    page.route("**/api/scrape/capture", handle)
    _inject_bookmarklet(page, ui_url, api_url, token, extra="&captureTimeout=800")

    message = page.locator("#ramekin-message")
    expect(message).to_contain_text("timed out", timeout=8_000)
    expect(page.locator("#ramekin-close")).to_be_visible()
    assert any("timed out" in line for line in logs), logs


def test_capture_stall_watchdog_surfaces(
    logged_in_page: Page, ui_url: str, api_url: str, fixture_base_url: str
):
    page = logged_in_page
    token = _get_token(page)

    logs: list[str] = []
    page.on("console", lambda msg: logs.append(msg.text))

    # Let the capture POST hit the real server (so a job id comes back), but
    # force every poll to report "pending" so the job never reaches a terminal
    # state and the overall stall watchdog must fire.
    def handle(route):
        if route.request.method == "GET":
            route.fulfill(
                status=200,
                headers={"Access-Control-Allow-Origin": "*"},
                content_type="application/json",
                body='{"status":"pending"}',
            )
        else:
            route.continue_()

    page.goto(f"{fixture_base_url}/seriouseats/rice_pilaf.html")
    page.route("**/api/scrape/**", handle)
    _inject_bookmarklet(
        page, ui_url, api_url, token, extra="&stallWarn=1500&pollInterval=300"
    )

    message = page.locator("#ramekin-message")
    expect(message).to_contain_text("Taking longer than expected", timeout=8_000)
    expect(page.locator("#ramekin-close")).to_be_visible()
    assert any("watchdog" in line for line in logs), logs
