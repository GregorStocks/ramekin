# Capture diagnostics: better logging for save-recipe

**Date:** 2026-06-18
**Status:** Approved (pending spec review)

## Problem

The web save-recipe bookmarklet frequently appears to hang on "Saving recipe…"
with nothing in the console indicating which step stalled or failed. The
bookmarklet is a personal tool; the author is comfortable using the browser
console/network tabs.

Root causes in `ramekin-ui/public/capture.js`:

- **No happy-path logging.** `console.error` only fires in the two `.catch`
  handlers. A normal or slow run logs nothing.
- **No timeout.** A hung capture `POST` or a hung poll `fetch` spins forever.
- **Static overlay text.** "Saving recipe…" never changes during the initial
  upload, so the visible phase doesn't reflect where time is being spent.

## Scope

- **In scope:** Web `capture.js` logging, staged overlay text, and a watchdog;
  a small iOS parity touch-up; a Playwright e2e test for the web flow.
- **Out of scope:**
  - iOS polling for parse completion/failure. iOS currently fires-and-forgets
    after the capture `POST` ("processed in the background"). Making it poll is
    a *feature* change, not diagnostics.
  - Full web overlay redesign. We keep the existing single-line overlay and
    only make its text phase-aware.
  - New API endpoints; `make pipeline` rerun (this does not change
    extraction/parsing behavior, so `data/` is unaffected).

## Current state

### Web (`ramekin-ui/public/capture.js`)
A self-contained IIFE served verbatim from `public/`. The bookmarklet shim
(`ramekin-ui/src/bookmarklet.js`, minified at runtime by `CreateRecipePage`)
injects `<script src="/capture.js?token=…&api=…&external=…">`. `capture.js`:
1. Parses `token` / `api` / `external` from its own `src`.
2. Builds the overlay (default message "Saving recipe…").
3. Captures `document.documentElement.outerHTML`.
4. `POST`s `{html, source_url}` to `/api/scrape/capture`.
5. Polls `GET /api/scrape/{id}` every 500ms until `status` is `completed`
   (shows View Recipe) or `failed` (shows the error). Non-terminal statuses map
   to "Extracting recipe…" (`parsing`) or "Processing…".

### iOS (already well-instrumented — minimal work)
`RamekinShareExtension/ShareViewController.swift` + `Shared/RamekinAPI.swift`
already:
- Log every step to a shared file via `DebugLogger` (timestamped, ms precision,
  viewable in the in-app Settings log viewer).
- Log HTML size: `captureHTML` logs `"… (html N bytes)"` plus
  `"SUCCESS: Capture job ID: …"`.
- Apply a request timeout (`captureSubmitTimeout`).
- Show a 10s "Still working, tap to close" slow-affordance watchdog.

The only gap vs. the new web behavior is that the share flow's capture call is
not timed, so a stall's *elapsed duration* isn't logged as a single line.

## Design

### 1. Web — logging helper + step logging
- Add `var t0 = Date.now();` and a small logger that prefixes
  `[Ramekin +<elapsed>ms]`:
  - `log(msg)` → `console.log`
  - `warn(msg)` → `console.warn`
  - `err(msg)` → `console.error`
- Replace the existing bare `console.error` calls so all output is uniform.
- Log each step:
  - init + token present
  - HTML captured, with size (`fmtBytes(html.length)`)
  - capture `POST` start
  - capture response status; on success, the returned job id
  - each poll: attempt number + the status it returned
  - terminal: completed (with recipe id) / failed (with error)

Example console output:

```
[Ramekin +0ms] init, token ok
[Ramekin +2ms] captured HTML 143 KB
[Ramekin +5ms] POST /api/scrape/capture…
[Ramekin +812ms] capture ok, job=abc123
[Ramekin +815ms] poll #1…
[Ramekin +1320ms] poll #1 -> parsing
[Ramekin +1825ms] poll #2 -> completed
```

`fmtBytes` formats `html.length` (character count — adequate for diagnostics)
as an approximate KB/MB string.

### 2. Web — staged overlay text
Make the overlay message track the phase rather than a static "Saving recipe…":

| Phase                          | Message                       |
|--------------------------------|-------------------------------|
| capture `POST` in flight       | `Uploading page (143 KB)…`    |
| job non-terminal, pending      | `Queued…`                     |
| job status `parsing`           | `Extracting recipe…`          |
| other non-terminal status `s`  | `Processing (s)…`             |
| completed                      | `Recipe saved!`               |

Append a live elapsed-seconds counter to the spinner line (e.g.
`Extracting recipe… (4s)`), updated by a 1s `setInterval`, cleared on any
terminal/error state. This makes the stuck phase and the passage of time
visible without opening devtools.

### 3. Web — watchdog (never silently spins)
Two layers:

- **Per-request timeout** via `AbortController`. The capture `POST` and each
  poll abort after a timeout. An abort rejects into the existing `.catch`,
  which already logs and shows error + Close.
- **Overall stall watchdog.** If the job is still non-terminal after a longer
  window, log a warning and switch the overlay to
  `Taking longer than expected (Ns) — check console/network` with a Close
  button, while polling continues in the background.

Timeout/threshold values are named constants at the top of the file. Proposed
defaults (tunable): capture `POST` 30s, per-poll 10s, overall stall warning
60s. **The constants are also overridable via query params** on the script
`src` (the same mechanism that already reads `token`/`api`/`external`). This
keeps the e2e watchdog test fast/deterministic and lets the author tweak
timeouts live while debugging.

### 4. iOS — timing touch-up (small, low-risk)
- Wrap the share extension's capture call in the existing
  `DebugLogger.timed("captureHTML", source: "ShareExtension")` so the call's
  duration is logged on both success and failure (today: start/size/success
  lines, but no single elapsed line — matching web's new elapsed-ms logging).
- Confirm a clear log line is written when the request timeout or the 10s
  slow-affordance trips; add one if missing.

No other iOS changes — size logging, step logging, timeout, watchdog, and the
in-app log viewer already exist.

## Testing

### Web — Playwright e2e (`tests/ui/`, Python + pytest)
Follows the existing pattern (`logged_in_page`, `fixture_base_url`,
`page.on("console")`).

- **Happy path:** open a fixture recipe page, inject `capture.js` with a valid
  token and the real `api` origin (simulating the bookmarklet). Assert the
  console emits the `[Ramekin +…ms]` step logs and the overlay progresses to
  "Recipe saved!" with a View Recipe link.
- **Watchdog/error path:** inject `capture.js` pointing `api` at a closed port
  (or with a tiny timeout via the new query-param override) and assert the
  overlay surfaces the timeout/stall error + Close button and the console logs
  a warning. The query-param timeout override keeps this fast and deterministic.

Open implementation detail to resolve in the plan: how the test obtains a
Bearer token (e.g. read it from the logged-in UI's storage, or mint one via the
login API).

### iOS
Verify via `make ios-build` and the in-app Settings log viewer (the existing
`SharedPagePayloadExtractorTests` etc. remain green). The `timed()` wrapper is
not meaningfully unit-testable on its own.

### Lint
`make lint` before the PR.

## Files touched

- `ramekin-ui/public/capture.js` — logging, staged overlay text, watchdog,
  query-param timeout overrides.
- `tests/ui/test_capture_bookmarklet.py` (new) — Playwright e2e.
- `ramekin-ios/RamekinShareExtension/ShareViewController.swift` — `timed()`
  wrapper around the capture call; ensure timeout/slow-affordance logging.
