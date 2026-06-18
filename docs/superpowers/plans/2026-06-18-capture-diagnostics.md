# Capture Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the save-recipe bookmarklet log every step (with elapsed timing + HTML size), show phase-aware overlay text, and never silently spin forever; bring iOS share-capture timing logging to parity.

**Architecture:** Rewrite the self-contained IIFE in `ramekin-ui/public/capture.js` to add a timestamped `[Ramekin +Nms]` logger, phase-aware overlay messages with a live elapsed counter, per-request `AbortController` timeouts, and an overall stall watchdog — all timeouts overridable via the script-src query params so e2e tests run fast. Add a Python+Playwright e2e covering the happy path and both watchdog paths. On iOS, wrap the share extension's capture call in the existing `DebugLogger.timed(...)` and log when the slow-affordance trips.

**Tech Stack:** Vanilla JS (served verbatim from `public/`), Python + Playwright (pytest, `tests/ui/`), Swift (iOS share extension).

**Spec:** `docs/superpowers/specs/2026-06-18-capture-diagnostics-design.md`

---

## File Structure

- **Modify** `ramekin-ui/public/capture.js` — full rewrite of the IIFE (logger, staged overlay, watchdog, query-param timeout overrides). Stays a single standalone file (served raw; cannot `import`).
- **Create** `tests/ui/test_capture_bookmarklet.py` — Playwright e2e: happy path, capture timeout, overall stall.
- **Modify** `ramekin-ios/RamekinShareExtension/ShareViewController.swift` — `timed()` wrapper around the capture call + slow-affordance log line.

Reference facts gathered while writing this plan:
- Server CORS is `CorsLayer::new().allow_origin(Any)` (`server/src/main.rs:226`) — cross-origin fetch from a fixture page to the API works.
- The UI stores its bearer token in `localStorage["token"]` (`ramekin-ui/src/context/AuthContext.tsx:82`).
- `capture.js` reads `token`/`api`/`external` from its own script `src`; `api`/`external` are `encodeURIComponent`-encoded by the builder and `decodeURIComponent`-decoded on read (`ramekin-ui/src/pages/CreateRecipePage.tsx:55-66`).
- Poll response shape: `{status, recipe_id, error}`; statuses include `pending`, `parsing`, `completed`, `failed` (`server/src/scraping/mod.rs:67-69`).
- iOS already logs HTML size + `NETWORK ERROR` on timeout; `captureSubmitTimeout = 25` (`ramekin-ios/Shared/RamekinAPI.swift:399,406`). `DebugLogger.timed(_:source:operation:)` exists (`ramekin-ios/Shared/DebugLogger.swift:56`).

---

## Task 1: Rewrite `capture.js`

**Files:**
- Modify: `ramekin-ui/public/capture.js` (replace entire file contents)

- [ ] **Step 1: Replace the file with the new implementation**

Write `ramekin-ui/public/capture.js` with exactly this content:

```js
(function () {
  // Find our script tag to get the token and API URL
  var scripts = document.getElementsByTagName("script");
  var thisScript = scripts[scripts.length - 1];
  var src = thisScript.src;
  var params = new URL(src).searchParams;
  var token = params.get("token");
  var origin = new URL(src).origin;
  var apiOrigin = decodeURIComponent(params.get("api") || origin);
  var externalOrigin = decodeURIComponent(params.get("external") || origin);

  // --- Diagnostics: timestamped logger -------------------------------------
  var t0 = Date.now();
  function elapsedMs() { return Date.now() - t0; }
  function elapsedSecs() { return Math.round(elapsedMs() / 1000); }
  function ts() { return "[Ramekin +" + elapsedMs() + "ms] "; }
  function log(m) { console.log(ts() + m); }
  function warn(m) { console.warn(ts() + m); }
  function err(m) { console.error(ts() + m); }

  // --- Tunable timeouts (overridable via bookmarklet query params) ---------
  function intParam(name, dflt) {
    var raw = params.get(name);
    if (raw === null) return dflt;
    var n = parseInt(raw, 10);
    return Number.isFinite(n) && n >= 0 ? n : dflt;
  }
  var CAPTURE_TIMEOUT_MS = intParam("captureTimeout", 30000);
  var POLL_TIMEOUT_MS = intParam("pollTimeout", 10000);
  var STALL_WARN_MS = intParam("stallWarn", 60000);
  var POLL_INTERVAL_MS = intParam("pollInterval", 500);

  function fmtBytes(n) {
    if (n < 1024) return n + " B";
    if (n < 1024 * 1024) return (n / 1024).toFixed(0) + " KB";
    return (n / (1024 * 1024)).toFixed(1) + " MB";
  }

  if (!token) {
    err("No token in bookmarklet URL");
    alert("Ramekin: Invalid bookmarklet. Please get a new one from your Ramekin account.");
    return;
  }

  // Don't run twice
  if (document.getElementById("ramekin-capture-overlay")) {
    return;
  }
  log("init, token ok; api=" + apiOrigin);

  // Capture HTML before we add our overlay
  var html = document.documentElement.outerHTML;
  var url = location.href;
  log("captured HTML " + fmtBytes(html.length));

  // Create overlay UI
  var overlay = document.createElement("div");
  overlay.id = "ramekin-capture-overlay";
  overlay.innerHTML = [
    '<div style="position:fixed;bottom:20px;right:20px;width:300px;padding:16px;',
    'background:#fff;border-radius:12px;box-shadow:0 8px 32px rgba(0,0,0,0.3);',
    'font-family:-apple-system,BlinkMacSystemFont,sans-serif;font-size:14px;',
    'z-index:2147483647;color:#333;">',
    '<div id="ramekin-status" style="display:flex;align-items:center;gap:8px;">',
    '<div id="ramekin-spinner" style="width:20px;height:20px;border:2px solid #e0e0e0;',
    'border-top-color:#4a9eff;border-radius:50%;animation:ramekin-spin 0.8s linear infinite;"></div>',
    '<span id="ramekin-message">Saving recipe...</span>',
    '</div>',
    '<div id="ramekin-actions" style="display:none;margin-top:12px;display:flex;gap:8px;"></div>',
    '<style>@keyframes ramekin-spin{to{transform:rotate(360deg)}}</style>',
    '</div>'
  ].join("");
  document.body.appendChild(overlay);

  var messageEl = document.getElementById("ramekin-message");
  var spinnerEl = document.getElementById("ramekin-spinner");
  var actionsEl = document.getElementById("ramekin-actions");

  // --- Overlay status helpers ---------------------------------------------
  var currentPhase = "Saving recipe...";
  var stalled = false;

  // Live elapsed-seconds counter on the spinner line.
  var timerId = setInterval(function () {
    if (stalled) return;
    if (spinnerEl.style.display !== "none") {
      messageEl.textContent = currentPhase + " (" + elapsedSecs() + "s)";
    }
  }, 1000);
  function stopTimer() {
    if (timerId) { clearInterval(timerId); timerId = null; }
  }

  function setPhase(text) {
    if (stalled) return;
    currentPhase = text;
    messageEl.textContent = text + " (" + elapsedSecs() + "s)";
  }

  function setStatus(message, isError, isDone) {
    stopTimer();
    messageEl.textContent = message;
    if (isError) {
      messageEl.style.color = "#d32f2f";
    }
    if (isDone || isError) {
      spinnerEl.style.display = "none";
    }
  }

  function showActions(recipeId) {
    actionsEl.style.display = "flex";
    actionsEl.innerHTML = [
      '<a href="', externalOrigin, '/recipes/', recipeId, '" target="_blank" ',
      'style="padding:8px 16px;background:#4a9eff;color:#fff;text-decoration:none;',
      'border-radius:6px;font-weight:500;">View Recipe</a>',
      '<button id="ramekin-close" style="padding:8px 16px;background:#e0e0e0;',
      'border:none;border-radius:6px;cursor:pointer;">Close</button>'
    ].join("");
    document.getElementById("ramekin-close").onclick = function () {
      overlay.remove();
    };
  }

  function showCloseButton() {
    actionsEl.style.display = "flex";
    actionsEl.innerHTML = [
      '<button id="ramekin-close" style="padding:8px 16px;background:#e0e0e0;',
      'border:none;border-radius:6px;cursor:pointer;">Close</button>'
    ].join("");
    document.getElementById("ramekin-close").onclick = function () {
      overlay.remove();
    };
  }

  // --- Networking with per-request timeout --------------------------------
  function fetchWithTimeout(resource, options, timeoutMs, label) {
    var controller = new AbortController();
    var timer = setTimeout(function () {
      warn(label + " timed out after " + timeoutMs + "ms - aborting");
      controller.abort();
    }, timeoutMs);
    options = options || {};
    options.signal = controller.signal;
    return fetch(resource, options).finally(function () {
      clearTimeout(timer);
    });
  }

  function handleFatal(label, timeoutMessage, e) {
    if (e && e.name === "AbortError") {
      warn(label + " aborted after timeout");
      setStatus(timeoutMessage, true);
    } else {
      err(label + " error: " + (e && e.message ? e.message : e));
      err("API origin: " + apiOrigin + " - if this looks like CORS, check the network tab");
      setStatus((e && e.message) || "Failed to save recipe", true);
    }
    showCloseButton();
  }

  // --- Stall watchdog ------------------------------------------------------
  var stallId = null;
  function startStallWatchdog() {
    stallId = setTimeout(function () {
      stalled = true;
      stopTimer();
      warn("watchdog: still not finished after " + STALL_WARN_MS + "ms");
      messageEl.textContent =
        "Taking longer than expected (" + elapsedSecs() + "s) - check console/network";
      messageEl.style.color = "#d32f2f";
      showCloseButton();
    }, STALL_WARN_MS);
  }
  function clearStallWatchdog() {
    if (stallId) { clearTimeout(stallId); stallId = null; }
  }

  // --- Poll loop -----------------------------------------------------------
  function pollJob(jobId, attempt) {
    log("poll #" + attempt + "...");
    fetchWithTimeout(apiOrigin + "/api/scrape/" + jobId, {
      headers: { "Authorization": "Bearer " + token }
    }, POLL_TIMEOUT_MS, "poll #" + attempt)
    .then(function (r) { return r.json(); })
    .then(function (job) {
      log("poll #" + attempt + " -> " + job.status);
      if (job.status === "completed" && job.recipe_id) {
        clearStallWatchdog();
        log("completed, recipe=" + job.recipe_id);
        setStatus("Recipe saved!", false, true);
        showActions(job.recipe_id);
      } else if (job.status === "failed") {
        clearStallWatchdog();
        warn("failed: " + (job.error || "unknown"));
        setStatus(job.error || "Failed to extract recipe", true);
        showCloseButton();
      } else {
        if (job.status === "parsing") {
          setPhase("Extracting recipe...");
        } else if (job.status === "pending") {
          setPhase("Queued...");
        } else {
          setPhase("Processing (" + job.status + ")...");
        }
        setTimeout(function () { pollJob(jobId, attempt + 1); }, POLL_INTERVAL_MS);
      }
    })
    .catch(function (e) {
      clearStallWatchdog();
      handleFatal("poll", "Status check timed out - check console/network", e);
    });
  }

  // --- Start the capture ---------------------------------------------------
  setPhase("Uploading page (" + fmtBytes(html.length) + ")");
  log("POST /api/scrape/capture...");
  fetchWithTimeout(apiOrigin + "/api/scrape/capture", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": "Bearer " + token
    },
    body: JSON.stringify({ html: html, source_url: url })
  }, CAPTURE_TIMEOUT_MS, "capture POST")
  .then(function (r) {
    log("capture response " + r.status);
    if (!r.ok) {
      return r.json().then(function (body) {
        throw new Error(body.error || ("Request failed (" + r.status + ")"));
      });
    }
    return r.json();
  })
  .then(function (result) {
    log("capture ok, job=" + result.id);
    setPhase("Queued...");
    startStallWatchdog();
    pollJob(result.id, 1);
  })
  .catch(function (e) {
    handleFatal("capture", "Upload timed out - check console/network", e);
  });
})();
```

- [ ] **Step 2: Lint the JS**

Run: `make lint`
Expected: PASS (no errors for `ramekin-ui/public/capture.js`). Fix any reported issues without using disable comments.

- [ ] **Step 3: Commit**

```bash
git add ramekin-ui/public/capture.js
git commit -m "Add step logging, staged overlay, and watchdog to capture.js"
```

---

## Task 2: Playwright e2e for the bookmarklet

**Files:**
- Create: `tests/ui/test_capture_bookmarklet.py`

- [ ] **Step 1: Write the test file**

Create `tests/ui/test_capture_bookmarklet.py` with exactly this content:

```python
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
    expect(page.get_by_role("button", name="Close")).to_be_visible()
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
    expect(page.get_by_role("button", name="Close")).to_be_visible()
    assert any("watchdog" in line for line in logs), logs
```

- [ ] **Step 2: Run the UI tests**

Run: `make test-ui`
Expected: PASS, including the three new `test_capture_bookmarklet` tests. (This boots the full process-compose stack — server, fixtures, mock OpenRouter, UI, seed — so it is slow.)

Troubleshooting if a new test fails:
- Happy path never reaches "Recipe saved!": confirm `seriouseats/rice_pilaf.html` completes for the mock extractor (it is the same fixture `tests/ui/test_scrape_status_page.py` drives to completion). If the mock no longer returns a recipe for it, switch to whichever fixture that test uses.
- Timeout/stall test flakes: a registered `page.route` handler that returns without `continue_/fulfill/abort` leaves the request paused (intended). If Playwright reports an unhandled route instead, fulfill the POST with a slow `route.fulfill` after the override window instead.

- [ ] **Step 3: Commit**

```bash
git add tests/ui/test_capture_bookmarklet.py
git commit -m "Add e2e tests for capture.js logging and watchdog"
```

---

## Task 3: iOS share-extension timing parity

**Files:**
- Modify: `ramekin-ios/RamekinShareExtension/ShareViewController.swift:306-349` (the `sendCapture` method)

- [ ] **Step 1: Add a slow-affordance log line**

In `sendCapture`, replace the slow-affordance `Task` block:

```swift
        Task {
            try? await Task.sleep(nanoseconds: UInt64(Self.slowAffordanceDelay * 1_000_000_000))
            await MainActor.run {
                if status == .sending {
                    showSlowAffordance = true
                }
            }
        }
```

with:

```swift
        Task {
            try? await Task.sleep(nanoseconds: UInt64(Self.slowAffordanceDelay * 1_000_000_000))
            await MainActor.run {
                if status == .sending {
                    DebugLogger.shared.log(
                        "Still sending after \(Int(Self.slowAffordanceDelay))s - showing slow affordance",
                        source: "ShareExtension"
                    )
                    showSlowAffordance = true
                }
            }
        }
```

- [ ] **Step 2: Wrap the capture call in `timed()` for duration logging**

Replace the capture `Task` block (the `do { ... } catch { ... }` that calls `RamekinAPI.shared.captureHTML`):

```swift
        Task {
            do {
                DebugLogger.shared.log("Calling RamekinAPI.shared.captureHTML...")
                _ = try await RamekinAPI.shared.captureHTML(
                    html: payload.html,
                    sourceURL: payload.url.absoluteString
                )
                DebugLogger.shared.log("API call completed successfully")
                logger.info("API call succeeded")

                await MainActor.run {
                    status = .success
                    DebugLogger.shared.log("Status set to success, will dismiss in 1.5s")
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                        DebugLogger.shared.log("Calling onComplete()")
                        onComplete()
                    }
                }
            } catch {
                DebugLogger.shared.log("API call FAILED: \(error)")
                DebugLogger.shared.log("Error localized: \(error.localizedDescription)")
                logger.error("API call failed: \(error.localizedDescription)")
                await MainActor.run {
                    status = .error
                    errorMessage = error.localizedDescription
                }
            }
        }
```

with:

```swift
        Task {
            do {
                _ = try await DebugLogger.shared.timed("captureHTML", source: "ShareExtension") {
                    try await RamekinAPI.shared.captureHTML(
                        html: payload.html,
                        sourceURL: payload.url.absoluteString
                    )
                }
                logger.info("API call succeeded")

                await MainActor.run {
                    status = .success
                    DebugLogger.shared.log("Status set to success, will dismiss in 1.5s", source: "ShareExtension")
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                        DebugLogger.shared.log("Calling onComplete()", source: "ShareExtension")
                        onComplete()
                    }
                }
            } catch {
                DebugLogger.shared.log("API call FAILED: \(error)", source: "ShareExtension")
                logger.error("API call failed: \(error.localizedDescription)")
                await MainActor.run {
                    status = .error
                    errorMessage = error.localizedDescription
                }
            }
        }
```

Rationale: `captureHTML` already logs its own start line with the HTML byte count and a success line with the job id; `timed()` adds a single `captureHTML completed (N.NNs)` / `captureHTML FAILED after N.NNs: ...` line so a stall's elapsed duration is visible — matching web's new elapsed-ms logging. The redundant manual "Calling..."/"completed"/"Error localized" lines are removed.

- [ ] **Step 3: Build the iOS app**

Run: `make ios-build`
Expected: PASS.
Note: this requires macOS + Xcode. If running on Linux, this step cannot execute here — the change is small and self-contained; flag to the user that it must be built on macOS/CI before merge.

- [ ] **Step 4: Commit**

```bash
git add ramekin-ios/RamekinShareExtension/ShareViewController.swift
git commit -m "Log capture duration and slow-affordance trip in iOS share extension"
```

---

## Task 4: Final lint + PR

- [ ] **Step 1: Run lint**

Run: `make lint`
Expected: PASS.

- [ ] **Step 2: Open the PR** via the create-pr workflow, describing the web logging/staging/watchdog changes, the e2e tests, and the iOS timing touch-up. Note the iOS build must be verified on macOS/CI if it could not run locally.

---

## Self-Review

**Spec coverage:**
- Logging helper + step logging → Task 1 (`log/warn/err`, `[Ramekin +Nms]`, every step). ✓
- HTML size → Task 1 (`fmtBytes(html.length)`). ✓
- Staged overlay text + live elapsed → Task 1 (`setPhase`, 1s interval). ✓
- Per-request timeout → Task 1 (`fetchWithTimeout` on capture + each poll). ✓
- Overall stall watchdog → Task 1 (`startStallWatchdog`). ✓
- Query-param timeout overrides → Task 1 (`intParam`). ✓
- iOS timing touch-up + slow-affordance log → Task 3. ✓
- Playwright e2e (happy + watchdog) → Task 2. ✓
- Lint before PR → Task 4. ✓
- Out of scope (iOS polling, overlay redesign, endpoints, pipeline) → not implemented, by design. ✓

**Placeholder scan:** No TBD/TODO; all steps contain complete code or exact commands.

**Type/name consistency:** `setPhase`/`setStatus`/`showActions`/`showCloseButton`/`fetchWithTimeout`/`handleFatal`/`startStallWatchdog`/`clearStallWatchdog`/`pollJob`/`stopTimer` defined once and called consistently. Query params `captureTimeout`/`pollTimeout`/`stallWarn`/`pollInterval` match between `capture.js` (`intParam`) and the test (`extra=`). iOS `timed(_:source:operation:)` signature matches `DebugLogger.swift:56`.
