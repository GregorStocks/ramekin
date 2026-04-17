# Design: External URL vs Self-Signed URL

## Problem

Two URL configuration concepts are currently tangled:

- `UI_HOSTNAME` — hostname used to pick an mkcert cert and configure the Vite dev server's HTTPS + `allowedHosts`.
- `RAMEKIN_QR_CODE_BASE_URL` — base URL baked into QR codes for PDF exports.

In practice, user-facing links (the bookmarklet's "View Recipe" button, the recipe URL shown in the capture overlay) fall back to whatever origin the UI happens to be loaded from — which is the self-signed URL. That sends users to an origin meant only for the HTTPS-terminated dev server, causing cert warnings and "wrong URL" experiences.

## Goals

- Introduce a clear separation: **external URL** (the One True Public URL, used for everything user-visible) vs **self-signed URL** (internal, used only for local HTTPS termination).
- Make the self-signed URL invisible to users outside of dev-server setup.
- Fail fast if the external URL is not configured.

## Env var changes

| Old | New | Form | Required |
|-----|-----|------|----------|
| `UI_HOSTNAME=localhost` | `RAMEKIN_SELF_SIGNED_URL=https://localhost:5173` | Full URL | Optional; defaults to `https://localhost:5173` |
| `RAMEKIN_QR_CODE_BASE_URL=...` | `RAMEKIN_EXTERNAL_URL=https://...` | Full URL | **Required** (build/dev startup fails if unset) |

`dev.env.example` sets `RAMEKIN_EXTERNAL_URL=https://localhost:5173` by default so local dev Just Works.

## Consumer changes

### 1. `scripts/setup-certs.sh`
Read `RAMEKIN_SELF_SIGNED_URL`, parse the hostname, pass to mkcert and use as cert dir name. Default to `https://localhost:5173` when unset.

### 2. `ramekin-ui/vite.config.ts`
- Parse `RAMEKIN_SELF_SIGNED_URL` → hostname for cert lookup (`~/.ramekin/certs/<hostname>/`) and for `server.allowedHosts`.
- Read `RAMEKIN_EXTERNAL_URL`; **throw** if missing. Inject as `__EXTERNAL_URL__` (replaces `__QR_CODE_BASE_URL__`).

### 3. `ramekin-ui/src/components/PdfExportModal.tsx`
Rename `qrCodeBaseUrl()` → `externalUrl()`. Read `__EXTERNAL_URL__`. Drop the `window.location.origin` fallback — it's required now.

### 4. `ramekin-ui/src/bookmarklet.js`
Add `external` query param to the capture.js script URL so it flows through to the page-side capture script:

```js
s.src = "__ORIGIN__/capture.js?token=__TOKEN__&api=__API__&external=__EXTERNAL__&t=" + Date.now();
```

### 5. `ramekin-ui/src/pages/CreateRecipePage.tsx`
When building the bookmarklet source, fill `__EXTERNAL__` with `encodeURIComponent(__EXTERNAL_URL__)`. `__ORIGIN__` and `__API__` keep using `window.location.origin` — the bookmarklet must talk to the Ramekin API over the self-signed origin (HTTPS + mkcert trust).

### 6. `ramekin-ui/public/capture.js`
Read the `external` param from the script tag's src; decode it. Use that value (not `origin`) when building the "View Recipe" `<a href>`. If the param is absent, fall back to `origin` so old bookmarklets don't break on reload (though they'll be regenerated the next time the user opens the bookmarklet page anyway).

### 7. `ramekin-ui/src/pages/CapturePage.tsx`
Line 200: build the recipe URL from `__EXTERNAL_URL__`, not `window.location.origin`.

### 8. Env files & CI
- `dev.env.example`, `test.env.example` — rename vars; set `RAMEKIN_EXTERNAL_URL=https://localhost:5173`.
- `.github/workflows/ci.yml` — rename `UI_HOSTNAME` line; add `RAMEKIN_EXTERNAL_URL` (e.g. `http://localhost:5173` to match the CI setup).

## Net effect

- Self-signed URL's job shrinks to: HTTPS cert setup (mkcert), Vite dev server `https` config, Vite `allowedHosts`. Nothing user-visible references it.
- External URL is the One True Public URL, baked into every user-facing link: QR codes, bookmarklet "View Recipe" buttons, recipe URLs shown in the capture overlay.
- The bookmarklet continues to talk to the API via the self-signed origin (it must, for HTTPS + cert trust), but every URL it *shows* the user points at the external URL.

## Out of scope

- No server-side changes. The Rust server doesn't reference `UI_HOSTNAME` or `RAMEKIN_QR_CODE_BASE_URL` today.
- No change to the bookmarklet's API-calling behavior.
- No new redirect logic on the server — the fix is UI-side, because that's where the wrong URLs were being constructed.
