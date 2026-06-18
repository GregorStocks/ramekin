# Bookmarklet auth hardening: long-lived scoped token + fail-fast capture

**Date:** 2026-06-18
**Status:** Approved (pending spec review)
**Follows:** `2026-06-18-capture-diagnostics-design.md` (logging/watchdog already landed in #575)

## Problem

The web save-recipe bookmarklet intermittently hangs for 30s on "Saving
recipe…" and then aborts, with no recipe saved. The new diagnostics (#575)
surfaced the failing run:

```
[Ramekin +3ms] POST /api/scrape/capture...
[Ramekin +30007ms] capture POST timed out after 30000ms - aborting
```

### Root cause (confirmed by logs + live reproduction)

Two coupled problems:

1. **The bookmarklet's token is being rejected (401).** The bookmarklet
   embeds the user's *live browser session token* (`CreateRecipePage.tsx`
   embeds `token()` from the auth context). Sessions expire after 30 days
   (sliding) and are also invalidated when the dev DB is reseeded by
   `make serve`'s seed step. A saved bookmarklet therefore goes stale and
   starts returning 401 — the real functional failure (no recipe is saved
   even when it doesn't hang).

2. **The 401 turns into a 30s hang via the dev/serve proxy.** Local dev/serve
   serves the UI through Vite (`vite preview` under `make serve`, `vite dev`
   under `make dev`), which proxies `/api/*` to the Rust backend over
   node-`http-proxy`. The browser streams the full ~500 KB body immediately
   (`fetch` does not use `Expect: 100-continue`). The backend's auth
   middleware rejects with 401 in ~5ms *before draining the request body*
   (`auth/middleware.rs` returns the moment the token check fails). The proxy
   is still writing the body into a backend socket that just closed →
   write-after-close → `[vite] http proxy error: /api/scrape/capture`, and it
   fails to relay the 401 back to the browser. The client sees nothing → the
   30s `AbortController` fires.

Evidence from `~/code/ramekin/logs/serve.log`:
- A hung capture logged `[vite] http proxy error` with **no** corresponding
  `[Vite Proxy] Response:` line.
- A capture that won the race logged `[Vite Proxy] Response: 401` and failed
  fast instead of hanging. The outcome is timing-dependent on body size.

Live reproduction (600 KB body, bad token, `Expect:` suppressed to mimic the
browser): straight to backend `:3000` → 401 in <1ms; through the `vite preview`
proxy `:5173` → 502/dropped. A capture that *authenticates* drains the whole
body (the handler reads it), so the proxy completes cleanly — which is why a
623 KB capture succeeded back in April: it was authorized.

This proxy layer is **local dev/serve only**; production reaches the backend
through its own reverse proxy. So the hang is primarily a local-workflow
problem, but the underlying "early error mid-upload" shape is real anywhere,
and the stale-token failure affects every environment.

### Why not a cookie?

Considered and rejected. The bookmarklet runs in a *third-party* page and
fetches the API cross-origin, so a cookie would have to be a third-party
cookie (`SameSite=None; Secure` + credentialed CORS, which also forces the
backend off its current `allow_origin(Any)`). Safari blocks third-party
cookies outright, Firefox by default, Chrome is removing them. Embedding a
token and sending it as a Bearer header — the current mechanism — is the
correct approach. The real issues are token *durability* and the *hang*, not
the transport.

## Goals

- A web bookmarklet that keeps working indefinitely (no silent 30-day expiry).
- An exposed long-lived credential that is **scoped** — capture only, not full
  account access.
- Capture that **fails fast and visibly** when the token is genuinely invalid,
  in dev and prod, on web and iOS.

## Non-goals

- iOS does not get a "bookmarklet token". The iOS share extension
  authenticates with the app's login session token from the keychain
  (`RamekinAPI`/`credentialStore`), re-obtained by logging into the app. There
  is no bookmarklet on iOS. iOS gets only the fail-fast guard.
- No general token-scopes subsystem. A single `token_type` enum with a
  hardcoded route allowlist is sufficient.
- No regenerate/revoke UI. Tokens are arbitrarily-many and long-lived; a
  soft-delete-based revocation path can be added later if one ever leaks.
- No `make pipeline` rerun — this changes auth/capture flow, not
  extraction/parsing, so `data/` is unaffected.

## Design

### 1. Long-lived scoped token (web)

Treat the bookmarklet token as an ordinary token that is **minted
differently** and **scoped to certain endpoints**.

**Storage.** Add a `token_type` column to `sessions`
(`'session'` default | `'bookmarklet'`). Bookmarklet tokens are hashed exactly
like session tokens (`hash_token`), so the auth lookup is unchanged in shape.
They are minted with a far-future `expires_at` (effectively non-expiring). The
existing sliding-expiry update in `get_user_from_token` only fires when a token
is within ~29 days of expiring, so it never touches a far-future bookmarklet
token — no change needed there. Arbitrarily many per user; no uniqueness
constraint.

Because a fresh token is minted every time the bookmarklet is generated and old
tokens are never invalidated, there is nothing to "re-display": previously
saved bookmarklets keep working, and the page simply embeds a new token. This
is what lets us keep hash-only storage without a show-once/`localStorage`
dance.

**Minting.** New endpoint `POST /api/users/bookmarklet-token`, callable **only
with a normal session token**. It inserts a fresh `'bookmarklet'` row for the
authenticated user and returns the plaintext token once
(`{ "token": "…" }`). `CreateRecipePage` calls it when the bookmarklet section
is opened — once per page visit, cached in a signal — and embeds the returned
token in the generated bookmarklet instead of `token()`.

This endpoint is deliberately **excluded** from the bookmarklet allowlist
(below), so a leaked bookmarklet token cannot mint more tokens or otherwise
escalate.

**Scope enforcement.** `get_user_from_token` returns the token's `token_type`
alongside the `User`. `require_auth` enforces: a `'bookmarklet'` token may only
reach an allowlist of (method, route), matched on the `MatchedPath` route
*pattern* (not the raw path, so `/api/scrape/{id}` matches any job id):

| Method | Route                   | Why                          |
|--------|-------------------------|------------------------------|
| POST   | `/api/scrape/capture`   | the capture upload           |
| GET    | `/api/scrape/{id}`      | web poll for job status      |
| GET    | `/api/users/me`         | the fail-fast pre-flight     |

`OPTIONS` continues to pass through `require_auth` before any token check
(unchanged). Any other route reached with a bookmarklet token returns `403`.
Session tokens remain unrestricted, so no existing behavior changes.

### 2. Fail-fast guard (web + iOS)

Before uploading the captured HTML, do one cheap `GET /api/users/me` with the
token:

- **Not OK (401/403/network):** show a clear message immediately and **do not**
  send the capture POST. Web overlay: *"Bookmarklet expired — get a new one
  from Ramekin"* with the existing Close button. iOS share sheet: an equivalent
  message via the existing status UI / `DebugLogger`.
- **OK:** proceed with the capture POST exactly as today.

Because the large upload only happens after auth is confirmed, the backend
never short-circuits mid-upload, so the proxy-choke hang cannot occur — in dev
*or* prod. This also gives a correct, specific error instead of a generic
timeout.

The pre-flight is the same small piece of control flow on both clients
(`GET /api/users/me`, branch on status). It is not the deterministic
input/output "pure logic" that `doc/client-logic-sharing.md` targets, but the
behavior will be mirrored in `capture.js` and the iOS share extension and
covered by tests on both sides; the duplication is called out in the PR.

## Components touched

**Server (Rust)**
- Migration: add `token_type` to `sessions` (default `'session'`); regenerate
  `schema.rs` via the makefile.
- `auth/db.rs`: mint a bookmarklet token (far-future expiry, `token_type =
  'bookmarklet'`); `get_user_from_token` returns `(User, token_type)`.
- `auth/middleware.rs`: enforce the bookmarklet allowlist via `MatchedPath`.
- `api/users/bookmarklet_token.rs` (new): `POST /api/users/bookmarklet-token`,
  session-token-only.

**Web (`ramekin-ui`)**
- `CreateRecipePage.tsx`: mint + embed a bookmarklet token instead of `token()`.
- `public/capture.js`: pre-flight `GET /api/users/me` before the capture POST;
  clear "expired" messaging on failure.

**iOS (`ramekin-ios`)**
- Share extension / `RamekinAPI`: pre-flight `GET /api/users/me` before the
  capture POST; surface a clear "logged out / expired" message.

## Testing

- **API e2e** (`tests/`): mint endpoint returns a working token; a bookmarklet
  token gets `2xx` on each allowlisted route and `403` on a representative
  non-allowlisted one (e.g. `GET /api/recipes`); the mint endpoint itself
  rejects a bookmarklet token.
- **Web Playwright** (`tests/ui/test_capture_bookmarklet.py`): extend the
  existing capture test to cover the pre-flight happy path, and add an
  expired/invalid-token case asserting the overlay shows the "expired" message
  fast (well under the 30s watchdog) and no capture POST is sent.
- **iOS**: unit/UI coverage for the pre-flight branch (clear message on
  unauthorized, normal flow on authorized), verified via `make ios-build` and
  the in-app log viewer.
- **Lint**: `make lint` before the PR.

## Open questions

None outstanding. Revocation and a "manage bookmarklet tokens" view are
explicitly deferred (YAGNI) given scoped, low-risk tokens.
