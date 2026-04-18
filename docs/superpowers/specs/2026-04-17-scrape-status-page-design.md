# Scrape Status Page

## Problem

When a user submits a URL to be scraped, the UI today collapses the multi-step backend pipeline into a single status string ("pending" / "scraping" / "parsing") and shows "success" (or an error) at the end. When a scrape explodes mid-pipeline, there is nothing in the UI that tells us which step failed or what it produced up to that point. The tracing spans and `step_outputs` table already contain this detail — it just isn't surfaced.

We want a dedicated page that shows the full step-by-step status as it runs, keeps that view available on success for post-mortem, and exposes the raw per-step output on demand so a broken scrape is debuggable without digging through logs.

## Goals

- A stable URL per scrape job that shows live, per-step status.
- Every pipeline step is visible with an icon, duration, and a short summary.
- Each step's raw output is reachable in one click when debugging.
- Both existing submit flows (`/recipes/new`, `/capture`) end up on this page.

## Non-goals

- Server-sent events / WebSockets. Polling is fine.
- iOS UI changes.
- A persisted user preference to auto-redirect past the status page.
- Modifying the pipeline steps themselves.

## Route and navigation

- New route `/scrape/:id` renders `ScrapeStatusPage`.
- `CreateRecipePage` (`/recipes/new`) and `CapturePage` (`/capture`) stop their inline polling and auto-populate logic. On a successful `createScrape` / `captureScrape` response, they call `navigate("/scrape/:id")`.
- The page stays put when the job reaches a terminal state. On `completed`, it shows a prominent "View Recipe →" button linking to `/recipes/{recipe_id}`. On `failed`, it shows the error, the failing step, and a `Retry` button (wired to the existing `POST /api/scrape/{id}/retry`).
- Ownership check matches the existing `/api/scrape/{id}` endpoint — 404 if the job does not belong to the caller.
- iOS is untouched for this change.

### Consequence for `/recipes/new`

The existing post-scrape "auto-populate an editable form, then save" flow on `CreateRecipePage` goes away as part of this change. The recipe is already persisted server-side by the `save_recipe` step, so the review-before-commit UX is not actually preventing a save today. After this change, the user lands on `/scrape/:id`, clicks "View Recipe →", and edits the recipe from the recipe detail page like any other recipe. This intentionally eliminates the dual "edit on new" / "edit on detail" flows.

## Backend API

### Extend `GET /api/scrape/{id}`

Add a `steps` array to the existing `ScrapeJobResponse`. No new top-level poll endpoint.

```
ScrapeJobResponse {
  id, status, url, recipe_id, error, failed_at_step, can_retry, retry_count,  // existing
  steps: [
    {
      name: string,          // e.g. "fetch_html"
      status: "pending" | "running" | "completed" | "failed" | "skipped",
      started_at?: timestamp,
      finished_at?: timestamp,
      duration_ms?: i64,
      summary?: string,      // short human line, e.g. "142 KB in 1.2s"
      error?: string,
      has_output: bool,      // true if a step_outputs row exists
    }
  ]
}
```

The `steps` array is always returned in the canonical pipeline order:

1. `fetch_html`
2. `extract_recipe`
3. `fetch_images`
4. `parse_ingredients`
5. `save_recipe`
6. `enrich_normalize_ingredients`
7. `enrich_auto_tag`
8. `apply_auto_tags`
9. `enrich_generate_photo`

Status derivation:

- Steps with a `completed` row in `step_outputs` are `completed`.
- The step named in `scrape_jobs.current_step` is `running`.
- The step named in `scrape_jobs.failed_at_step` is `failed`.
- Steps that precede the terminal failing step but have no output row are `skipped` (shouldn't happen in the normal pipeline, but handles retries that jump past finished work).
- Steps after `current_step` / `failed_at_step` are `pending`.

`summary` is produced by a small server-side helper `step_summary(step_name, output) -> Option<String>` that formats a short line from the fields the step already records (HTML byte count, extracted recipe title, `n/m images succeeded`, etc.). It lives in one file so formatting isn't scattered across pipeline code.

### New endpoint `GET /api/scrape/{id}/steps/{step_name}/output`

Returns the raw `step_outputs` row for a given step as JSON. Ownership-checked like the existing scrape endpoints. Returns 404 if the step has no row yet. Used by the UI when the user expands a step row for deep debugging.

### Database

Verified during planning:

- `scrape_jobs` already has `current_step`, `failed_at_step`, `status`, `created_at`, `updated_at`, `retry_count`, `error_message` (not `error`). There is no `completed_at`.
- `step_outputs` is append-only with `created_at` only (no `started_at` / `finished_at`).

One small migration: add `current_step_started_at TIMESTAMPTZ NULL` to `scrape_jobs`, written whenever `current_step` is written. Per-step timing is then derived:

- Completed step: `finished_at = step_outputs.created_at`, `started_at = finished_at - duration_ms` (duration is stored in the output JSON by the existing `StepResult`).
- Running step: `started_at = scrape_jobs.current_step_started_at`, `finished_at = null`.

### Retry behavior

Confirmed during planning: `POST /api/scrape/{id}/retry` mutates the same job ID (does not create a new one). The UI stays on `/scrape/:id`; polling resumes with the reset status.

## Frontend

### `ScrapeStatusPage` component

- Reuses the existing polling pattern (1 second interval, stops at terminal). Lives in `ramekin-ui/src/pages/ScrapeStatusPage.tsx`.
- Header: scrape URL, overall status pill, elapsed time since `created_at`.
- Body: vertical list of step rows.

### Step row

Each row shows:

- Status icon: `○` pending, `◐` (animated) running, `✓` completed, `✗` failed, `−` skipped.
- Step name (e.g. "fetch_html").
- Duration if known (e.g. "1.2s").
- `summary` line if present.
- Error text below the row if the step failed.

Each row is expandable. On expand, it lazy-fetches `GET /api/scrape/{id}/steps/{name}/output` and renders the JSON in a `<pre>` block. Plain `<pre>`, not a tree widget — no new dependency. If `has_output` is false, the row is not expandable.

### Terminal states

- `completed`: green "View Recipe →" button at the bottom linking to `/recipes/{recipe_id}`.
- `failed`: error banner at the top, "Retry" button wired to `POST /api/scrape/{id}/retry`. The retry endpoint reuses the same job ID, so polling resumes on the same page with the job's status reset.

## Testing

Per `AGENTS.md`, end-to-end tests are written before using new API endpoints in the UI.

- Rust unit tests: `step_summary` formatting per step type (success path, no-output path).
- Rust integration tests:
  - Extended `GET /api/scrape/{id}` returns the new `steps` array with the correct shape for pending / running / completed / failed jobs.
  - `GET /api/scrape/{id}/steps/{name}/output` returns output for a completed step, 404 for a step with no row, 404 for a non-owner.
- End-to-end test (`tests/test_scrape.py` or sibling): submit a scrape against a fixture URL, poll `/api/scrape/{id}`, assert steps progress `pending → running → completed`, and assert `/steps/fetch_html/output` returns the stored HTML fetch output.
- UI smoke test: `/scrape/:id` renders the step list, an expand click triggers the output fetch, terminal states render the correct action button.

## Migration / rollout

No feature flag. Single PR lands the backend changes, the new route, and the updates to `CreateRecipePage` / `CapturePage`. There is no production environment, so no backwards-compatibility work is needed. `make pipeline` is not affected — pipeline outputs are unchanged.
