# Shopping-list keyboard latency fix + client log upload

**Date:** 2026-07-02
**Status:** Approved (pending spec review)

## Problem

1. On iOS, tapping "+" on the Shopping List screen takes noticeably long to
   bring up the keyboard.
2. There is no way for an agent to diagnose client-side performance issues
   autonomously: iOS logs stay on-device (manual ShareLink export only) and the
   web client has no logging at all.

### Diagnosis of (1)

`ramekin-ios/Ramekin/ShoppingListView.swift` toolbar "+" button sets
`isAddingItem = true` and `addFieldFocused = true` in the same SwiftUI update.
The `@FocusState` write targets a `TextField` that is not yet in the view
hierarchy (the add section is inserted *because of* `isAddingItem`), so SwiftUI
drops or defers the focus request; the keyboard only appears once focus
eventually lands. When the list is empty there is extra work: the tap also
swaps `emptyState` for a freshly built `List`.

The web client has no equivalent bug: its add input is always rendered and the
browser focuses it directly on tap. The keyboard fix is therefore iOS-only
(confirmed with user); the instrumentation covers both clients.

## Scope

- **In scope:**
  - iOS: fix the focus timing; add tap→keyboard latency measurement.
  - Server: authenticated client-log upload endpoint + table + e2e tests.
  - iOS: "Upload Logs" button in Settings posting the existing `DebugLogger`
    file to the server.
  - Web: minimal ring-buffer logger (`log`/`warn`/`error`/`timed`), initial
    instrumentation of the shopping-list add path, and an "Upload debug logs"
    button on the existing Settings page.
- **Out of scope:**
  - Automatic/background log upload, crash reporting, third-party telemetry
    (Sentry etc.).
  - Retention/cleanup jobs for uploaded logs (soft-delete policy applies; no
    hard deletes anyway).
  - Broad instrumentation of every web page — only the shopping-list path and
    conversion of the existing bare `console.error` call sites.
  - `make pipeline` rerun — no extraction/parsing behavior changes.

## Design

### 1. iOS — keyboard fix (`ShoppingListView.swift`)

- The "+" button sets only `isAddingItem = true`.
- The "Ingredient" `TextField` gains `.onAppear { addFieldFocused = true }`,
  so focus is requested only once the field exists. This also covers the
  empty-list case (the whole `List` mounts, then the field appears).
- `addItem()`'s re-focus (`addFieldFocused = true` while the field is already
  mounted) is unaffected.

### 2. iOS — tap→keyboard latency measurement

In `ShoppingListView`:

- On "+" tap, record `CFAbsoluteTimeGetCurrent()` in a `@State` var and log
  `"add tapped"` via `DebugLogger` (source `"Shopping"`).
- Subscribe with
  `.onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardDidShowNotification))`;
  when a tap timestamp is pending, log
  `"keyboard shown +<ms>ms after add tap"` and clear the timestamp.

This lands **before** the focus fix in the implementation order so the fix has
a before/after number.

### 3. Server — client log upload

**Migration** (soft-delete like all tables): `client_log_uploads`

| column        | type                    |
|---------------|-------------------------|
| `id`          | uuid PK                 |
| `user_id`     | uuid FK → users         |
| `platform`    | text (`"ios"`/`"web"`)  |
| `app_version` | text nullable           |
| `os_info`     | text nullable           |
| `content`     | text                    |
| `created_at`  | timestamptz             |
| `deleted_at`  | timestamptz nullable    |

**Endpoints** (authenticated, user-scoped, Diesel DSL only):

- `POST /api/client-logs` — body `{platform, app_version?, os_info?, content}`
  → `201 {id}`. Reject `content` over 2 MB (matches the iOS log-rotation cap)
  with a 4xx — fail fast, no truncation.
- `GET /api/client-logs` — newest-first list of the caller's uploads,
  metadata only (`id`, `platform`, `app_version`, `os_info`, `created_at`,
  content length).
- `GET /api/client-logs/{id}` — full record including `content`. 404 if not
  the caller's or soft-deleted.

E2E tests land before either client uses the endpoints (repo rule). OpenAPI
spec/clients regenerate via the normal build; generated code is not hand-edited.

**Agent retrieval path:** the GET endpoints. In dev, via the seeded test user;
against a real server, via an authenticated call with the user's credentials.

### 4. iOS — upload button (`SettingsView.swift`)

- "Upload Logs to Server" button next to the existing log viewer/ShareLink.
- Reads `DebugLogger.shared.readLogs()`, posts to `POST /api/client-logs`
  through the same API layer the rest of the app uses, with
  `platform: "ios"`, `app_version` from `Bundle.main`, `os_info` from
  `UIDevice`.
- Shows a success confirmation or the error verbatim (fail fast, no silent
  catch). Logs are *not* cleared on upload — the existing Clear button stays
  the only way to clear.

### 5. Web — ring-buffer logger (`ramekin-ui/src/utils/logger.ts`)

- Module-level singleton holding the last 1000 entries:
  `{timestamp, level, source, message}`.
- API mirrors the iOS logger's shape: `log(source, msg)`, `warn`, `error`,
  and `timed(source, label, fn)` (async wrapper logging start + elapsed ms +
  success/failure). Entries also mirror to the corresponding `console.*`.
- `dump()` returns the buffer formatted as text lines
  (`2026-07-02T18:03:12.345Z [Shopping] …`) for upload.
- This is platform infrastructure, not shared deterministic logic — no shared
  test vectors needed per `doc/client-logic-sharing.md`.

**Initial call sites:**

- `ShoppingListPage.tsx`: wrap `createItems` and the follow-up `listItems`
  refetch in `timed()`; log add-form submit.
- Replace the existing bare `console.error` calls in `CapturePage.tsx` and
  `ImportPage.tsx` with `logger.error`.

### 6. Web — upload button (`SettingsPage.tsx`)

- "Upload debug logs" button posting `logger.dump()` to `POST /api/client-logs`
  via the generated API client, with `platform: "web"`, `os_info` from
  `navigator.userAgent` (no app_version). Success/error surfaced inline.

## Error handling

- Server rejects oversized or empty `content`, unknown `platform` values, and
  unauthenticated calls — no lenient coercion.
- Both upload buttons surface failures to the user; nothing retries silently.

## Testing

- **Server:** e2e tests (existing API-test harness, `make test`) covering
  create → list → get round-trip, user scoping (user B cannot read user A's
  upload), size-limit rejection, and bad-platform rejection.
- **Web:** Vitest unit tests for the ring buffer (capacity eviction, `timed`
  success/failure, `dump` format) via `make ui-unit-test`.
- **iOS:** `make ios-build` + manual verification; keyboard latency verified
  by the new perf-mark log lines before and after the focus fix (numbers go in
  the PR description).
- `make lint` before the PR.

## Files touched

- `migrations/…` (new) — `client_log_uploads` table.
- `server/src/…` — routes/handlers/models for the three endpoints.
- `tests/…` — e2e tests for the endpoints.
- `ramekin-ios/Ramekin/ShoppingListView.swift` — focus fix + latency marks.
- `ramekin-ios/Ramekin/SettingsView.swift` — upload button.
- `ramekin-ui/src/utils/logger.ts` (new) + unit tests.
- `ramekin-ui/src/pages/ShoppingListPage.tsx` — instrumentation.
- `ramekin-ui/src/pages/SettingsPage.tsx` — upload button.
- `ramekin-ui/src/pages/CapturePage.tsx`, `ImportPage.tsx` — `console.error` →
  logger.
