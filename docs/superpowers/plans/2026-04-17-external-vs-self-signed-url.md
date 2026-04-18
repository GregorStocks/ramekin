# External URL vs Self-Signed URL Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate the self-signed dev HTTPS URL from the public-facing "external" URL so user-visible links (QR codes, bookmarklet "View Recipe", capture overlay recipe URLs) never point at the self-signed origin.

**Architecture:** Two env vars with clearly distinct jobs. `RAMEKIN_SELF_SIGNED_URL` (optional, defaults to `https://localhost:5173`) is used only by mkcert setup and the Vite dev server. `RAMEKIN_EXTERNAL_URL` (required) is baked into the UI bundle as `__EXTERNAL_URL__` and flows through every user-visible URL path — including out through the bookmarklet loader into `capture.js` so the "View Recipe" button opens the public URL, not the self-signed one.

**Tech Stack:** Vite (dev server, bundler), SolidJS (UI), bash (cert setup), GitHub Actions (CI env files).

**Spec:** `docs/superpowers/specs/2026-04-17-external-vs-self-signed-url-design.md`

---

## File Structure

**Modify:**
- `scripts/setup-certs.sh` — read `RAMEKIN_SELF_SIGNED_URL`, parse hostname.
- `ramekin-ui/vite.config.ts` — parse self-signed URL, require external URL, inject `__EXTERNAL_URL__`.
- `ramekin-ui/src/components/PdfExportModal.tsx` — rename helper, drop fallback.
- `ramekin-ui/src/bookmarklet.js` — add `external=__EXTERNAL__` param to script URL.
- `ramekin-ui/src/pages/CreateRecipePage.tsx` — fill new `__EXTERNAL__` placeholder.
- `ramekin-ui/public/capture.js` — read `external` param, use it for "View Recipe" href.
- `ramekin-ui/src/pages/CapturePage.tsx` — build recipe URL from `__EXTERNAL_URL__`.
- `dev.env.example`, `test.env.example`, `.github/workflows/ci.yml` — rename env vars, set `RAMEKIN_EXTERNAL_URL`.

**No server-side changes.** The Rust server does not reference either env var.

---

## Task 1: Rename env var in dev/test env files

**Files:**
- Modify: `dev.env.example:6`, `dev.env.example:25-28`
- Modify: `test.env.example:7`

- [ ] **Step 1: Edit `dev.env.example`**

Replace line 6 `UI_HOSTNAME=localhost` with:

```
RAMEKIN_SELF_SIGNED_URL=https://localhost:5173
```

Replace lines 25-28 (the `RAMEKIN_QR_CODE_BASE_URL` block) with:

```
# External URL used for all user-facing links: QR codes, bookmarklet "View Recipe"
# buttons, recipe URLs shown in the capture overlay. Required — dev/build fails
# if unset. In local dev this points at the self-signed UI origin.
RAMEKIN_EXTERNAL_URL=https://localhost:5173
```

- [ ] **Step 2: Edit `test.env.example`**

Replace line 7 `UI_HOSTNAME=localhost` with:

```
RAMEKIN_SELF_SIGNED_URL=https://localhost:5174
RAMEKIN_EXTERNAL_URL=http://localhost:5174
```

(Note: test env uses `UI_PORT=5174`; tests run without certs, so external uses `http://`.)

- [ ] **Step 3: Commit**

```bash
git add dev.env.example test.env.example
git commit -m "Rename UI_HOSTNAME and RAMEKIN_QR_CODE_BASE_URL in env examples"
```

---

## Task 2: Update `scripts/setup-certs.sh` to read new env var

**Files:**
- Modify: `scripts/setup-certs.sh:4,9`

- [ ] **Step 1: Edit the comment and the HOSTNAME line**

Replace lines 2-10 of `scripts/setup-certs.sh`:

```bash
#!/bin/bash
# Generate mkcert certificates for local HTTPS development
# Certs are stored in ~/.ramekin/certs/{hostname}/
# Uses RAMEKIN_SELF_SIGNED_URL env var, defaults to https://localhost:5173

set -e

CERT_BASE="$HOME/.ramekin/certs"
SELF_SIGNED_URL="${RAMEKIN_SELF_SIGNED_URL:-https://localhost:5173}"
# Strip scheme, port, and path to get bare hostname
HOSTNAME="$(echo "$SELF_SIGNED_URL" | sed -E 's#^[a-z]+://([^:/]+).*#\1#')"
CERT_DIR="$CERT_BASE/$HOSTNAME"
```

- [ ] **Step 2: Verify setup-certs.sh still runs**

```bash
RAMEKIN_SELF_SIGNED_URL=https://localhost:5173 bash scripts/setup-certs.sh
```

Expected: prints either "Certs already exist for localhost" or regenerates them in `~/.ramekin/certs/localhost/`.

- [ ] **Step 3: Commit**

```bash
git add scripts/setup-certs.sh
git commit -m "Read RAMEKIN_SELF_SIGNED_URL in setup-certs.sh, parse hostname"
```

---

## Task 3: Update `vite.config.ts` — parse self-signed URL, require external URL, rename injected constant

**Files:**
- Modify: `ramekin-ui/vite.config.ts:9-10,52,58`

- [ ] **Step 1: Replace the URL-parsing block at the top**

Replace lines 9-10 with:

```ts
const selfSignedUrl = process.env.RAMEKIN_SELF_SIGNED_URL || 'https://localhost:5173'
const hostname = new URL(selfSignedUrl).hostname
const certDir = path.join(process.env.HOME || '', '.ramekin', 'certs', hostname)
```

- [ ] **Step 2: Require `RAMEKIN_EXTERNAL_URL`, rename the constant**

Replace line 52:

```ts
const externalUrl = process.env.RAMEKIN_EXTERNAL_URL
if (!externalUrl) {
  throw new Error('RAMEKIN_EXTERNAL_URL is required (see dev.env.example)')
}
```

Replace line 58 (inside the `define` block):

```ts
    __EXTERNAL_URL__: JSON.stringify(externalUrl),
```

- [ ] **Step 3: Verify Vite config throws when external URL is unset**

```bash
cd ramekin-ui && RAMEKIN_EXTERNAL_URL= npx vite build 2>&1 | head -20
```

Expected: build fails with `Error: RAMEKIN_EXTERNAL_URL is required`.

- [ ] **Step 4: Verify Vite config succeeds when set**

```bash
cd ramekin-ui && RAMEKIN_EXTERNAL_URL=https://localhost:5173 UI_PORT=5173 PORT=3000 npx vite build 2>&1 | tail -10
```

Expected: build succeeds.

- [ ] **Step 5: Commit**

```bash
git add ramekin-ui/vite.config.ts
git commit -m "Rename Vite config URL vars; require RAMEKIN_EXTERNAL_URL"
```

---

## Task 4: Update `PdfExportModal.tsx` — rename helper, drop fallback

**Files:**
- Modify: `ramekin-ui/src/components/PdfExportModal.tsx:7,190-198`

- [ ] **Step 1: Rename the declared constant**

Replace line 7:

```ts
declare const __EXTERNAL_URL__: string;
```

- [ ] **Step 2: Rename the helper and drop the fallback**

Replace lines 190-198 (`qrCodeBaseUrl` + `recipeUrlFor`):

```ts
function externalUrl(): string {
  return __EXTERNAL_URL__.replace(/\/+$/, "");
}

function recipeUrlFor(recipeId: string): string {
  return `${externalUrl()}/recipes/${recipeId}`;
}
```

- [ ] **Step 3: Run TypeScript check**

```bash
cd ramekin-ui && npx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add ramekin-ui/src/components/PdfExportModal.tsx
git commit -m "Rename qrCodeBaseUrl() to externalUrl(), drop fallback"
```

---

## Task 5: Thread external URL through the bookmarklet loader

**Files:**
- Modify: `ramekin-ui/src/bookmarklet.js:3`
- Modify: `ramekin-ui/src/pages/CreateRecipePage.tsx:56-65`

- [ ] **Step 1: Add `external` param to bookmarklet.js**

Replace line 3:

```js
  s.src = "__ORIGIN__/capture.js?token=__TOKEN__&api=__API__&external=__EXTERNAL__&t=" + Date.now();
```

- [ ] **Step 2: Fill the placeholder in `CreateRecipePage.tsx`**

Replace lines 56-65 (the `bookmarkletCode` memo through the `.replace(...)` chain):

```tsx
  const bookmarkletCode = createMemo(() => {
    const origin = window.location.origin;
    // Use UI origin for API calls - Vite proxy forwards /api/* to the API server
    const apiOrigin = origin;
    const userToken = token();
    if (!userToken) return "";
    const code = bookmarkletSource
      .replace("__ORIGIN__", origin)
      .replace("__TOKEN__", userToken)
      .replace("__API__", encodeURIComponent(apiOrigin))
      .replace("__EXTERNAL__", encodeURIComponent(__EXTERNAL_URL__));
```

- [ ] **Step 3: Add the `__EXTERNAL_URL__` declaration if missing**

Check the top of `ramekin-ui/src/pages/CreateRecipePage.tsx`. If there is no `declare const __EXTERNAL_URL__: string;` already, add it after the imports:

```ts
declare const __EXTERNAL_URL__: string;
```

- [ ] **Step 4: Run TypeScript check**

```bash
cd ramekin-ui && npx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add ramekin-ui/src/bookmarklet.js ramekin-ui/src/pages/CreateRecipePage.tsx
git commit -m "Thread external URL through bookmarklet loader"
```

---

## Task 6: Use external URL for "View Recipe" link in `capture.js`

**Files:**
- Modify: `ramekin-ui/public/capture.js:8-9,63`

- [ ] **Step 1: Parse the `external` param and use it for the "View Recipe" href**

Replace lines 8-9:

```js
  var origin = new URL(src).origin;
  var apiOrigin = decodeURIComponent(params.get("api") || origin);
  var externalOrigin = decodeURIComponent(params.get("external") || origin);
```

Replace the `showActions` function's first innerHTML line (currently line 63):

```js
      '<a href="', externalOrigin, '/recipes/', recipeId, '" target="_blank" ',
```

- [ ] **Step 2: Commit**

```bash
git add ramekin-ui/public/capture.js
git commit -m "Use external URL for bookmarklet View Recipe link"
```

---

## Task 7: Use external URL in `CapturePage.tsx`

**Files:**
- Modify: `ramekin-ui/src/pages/CapturePage.tsx:1,199-202`

- [ ] **Step 1: Declare `__EXTERNAL_URL__` at the top**

After the imports in `ramekin-ui/src/pages/CapturePage.tsx` (after line 2), add:

```ts
declare const __EXTERNAL_URL__: string;
```

- [ ] **Step 2: Replace `window.location.origin` with the external URL**

Replace line 200 (inside `handleViewRecipe`):

```tsx
    const recipeUrl = `${__EXTERNAL_URL__.replace(/\/+$/, "")}/recipes/${recipeId}`;
```

- [ ] **Step 3: Run TypeScript check**

```bash
cd ramekin-ui && npx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add ramekin-ui/src/pages/CapturePage.tsx
git commit -m "Use external URL for recipe URL in capture flow"
```

---

## Task 8: Update CI env files

**Files:**
- Modify: `.github/workflows/ci.yml:113`

- [ ] **Step 1: Rename `UI_HOSTNAME` and add `RAMEKIN_EXTERNAL_URL`**

Replace line 113:

```yaml
          echo "RAMEKIN_SELF_SIGNED_URL=http://localhost:${UI_PORT}" >> test-ui.env
          echo "RAMEKIN_EXTERNAL_URL=http://localhost:${UI_PORT}" >> test-ui.env
```

(CI runs without mkcert, so Vite falls back to HTTP — hence `http://`.)

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "Rename UI_HOSTNAME and add RAMEKIN_EXTERNAL_URL in CI env"
```

---

## Task 9: Update developer-facing env files in the worktree & verify end-to-end

**Files:**
- Modify: `dev.env`, `test.env` (if they exist locally — these are gitignored copies of `.env.example` files)

- [ ] **Step 1: Check which env files exist**

```bash
ls -la /Users/gregorstocks/code/worktrees/pursuable-banister/dev.env /Users/gregorstocks/code/worktrees/pursuable-banister/test.env 2>&1
```

- [ ] **Step 2: If `dev.env` exists, update it to match the new naming**

For any `dev.env` present:

```bash
# replace UI_HOSTNAME with RAMEKIN_SELF_SIGNED_URL
sed -i.bak 's|^UI_HOSTNAME=.*|RAMEKIN_SELF_SIGNED_URL=https://localhost:5173|' dev.env
# replace RAMEKIN_QR_CODE_BASE_URL if present, add RAMEKIN_EXTERNAL_URL if not
if grep -q '^RAMEKIN_QR_CODE_BASE_URL' dev.env; then
  sed -i.bak2 's|^RAMEKIN_QR_CODE_BASE_URL=.*|RAMEKIN_EXTERNAL_URL=https://localhost:5173|' dev.env
else
  echo "RAMEKIN_EXTERNAL_URL=https://localhost:5173" >> dev.env
fi
rm -f dev.env.bak dev.env.bak2
```

Apply the equivalent treatment to `test.env` using port `5174` and `http://` (matching Task 1 step 2).

- [ ] **Step 3: Run the linter**

```bash
make lint
```

Expected: passes.

- [ ] **Step 4: Run UI tests**

```bash
make test-ui
```

Expected: passes. If it fails due to the env rename not propagating, double-check `test.env` and `.github/workflows/ci.yml`.

- [ ] **Step 5: Start the dev server and manually verify the bookmarklet flow**

```bash
make dev-headless
```

In a browser, log in, go to the create-recipe page, drag the bookmarklet to the toolbar, visit an external recipe page, trigger the bookmarklet. When capture completes, click "View Recipe" in the overlay.

Expected: the "View Recipe" link opens `https://localhost:5173/recipes/<id>` (which is the external URL in local dev). Confirm in the URL bar.

Also exercise PDF export (double-sided) on a recipe and inspect the generated QR code points at `https://localhost:5173/recipes/<id>`.

- [ ] **Step 6: Stop the dev server and commit any local env file changes**

```bash
make dev-down
```

No commit needed if only gitignored local env files changed; all source changes have been committed in prior tasks.

---

## Final verification

- [ ] **Step 1: Confirm no lingering references to the old names**

```bash
grep -rn "UI_HOSTNAME\|QR_CODE_BASE_URL\|__QR_CODE_BASE_URL__\|qrCodeBaseUrl" \
    --include='*.ts' --include='*.tsx' --include='*.js' --include='*.sh' \
    --include='*.yml' --include='*.env.example' \
    .
```

Expected: no matches.

- [ ] **Step 2: Confirm all new names are present in expected files**

```bash
grep -rn "RAMEKIN_SELF_SIGNED_URL\|RAMEKIN_EXTERNAL_URL\|__EXTERNAL_URL__\|externalUrl" \
    --include='*.ts' --include='*.tsx' --include='*.js' --include='*.sh' \
    --include='*.yml' --include='*.env.example' \
    .
```

Expected: matches in the files listed in "File Structure".

- [ ] **Step 3: Run `make lint` one more time**

```bash
make lint
```

Expected: passes.
