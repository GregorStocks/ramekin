# Warmer Cookbook Aesthetic — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the generic dark Vite-starter look with a warm light editorial cookbook aesthetic: Fraunces display serif + Plus Jakarta Sans body, cream surfaces, saffron accent, calm interactions, plus a token foundation that eliminates ~340 hardcoded hex literals in `App.css`.

**Architecture:** All design tokens live at `:root` in `ramekin-ui/src/index.css`. `App.css` consumes tokens only — no literal color values post-refactor. Light-only theme; the prior dark-mode `@media (prefers-color-scheme: light)` block is removed. Fonts load from Google Fonts via `<link>` in `index.html`. Paper-grain is a fixed body `::before`.

**Tech Stack:** SolidJS + Vite + TypeScript, Stylelint, Playwright smoke tests, Google Fonts (Fraunces, Plus Jakarta Sans).

**Spec:** `docs/agent/2026-04-17-warmer-cookbook-aesthetic-design.md`

**Guardrail throughout:** every commit must leave `make lint` green and the dev server loading without console errors. The Playwright smoke in `tests/ui/test_smoke.py` must keep passing. These are the regression tripwires — run them between tasks.

---

## File Structure

**Modified:**
- `ramekin-ui/index.html` — add Google Fonts `<link>` + preconnect.
- `ramekin-ui/src/index.css` — replace dark-mode `:root` + light media query with the full token set and light-mode base styles.
- `ramekin-ui/src/App.css` — section-by-section sweep replacing hex literals with tokens, applying new palette/typography.
- `ramekin-ui/.stylelintrc.json` — add a rule forbidding raw hex literals in `App.css` (tokens allowed only in `index.css`).

**Created:**
- `ramekin-ui/src/assets/grain.png` — 200×200 tileable 8-bit noise, ≤10 KB.

**Deleted at the end:**
- `issues/p3-warmer-cookbook-aesthetic.json5`.

No TSX or component-structure changes. Inline `style={{ ... }}` usages in pages are all layout/utility (flex, opacity, margin) and unaffected.

---

### Task 1: Load fonts + preconnect

**Files:**
- Modify: `ramekin-ui/index.html`

- [ ] **Step 1: Add Google Fonts link + preconnect**

Replace the `<head>` so it looks like:

```html
<head>
  <meta charset="UTF-8" />
  <link rel="icon" type="image/svg+xml" href="/vite.svg" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Ramekin</title>
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
  <link
    href="https://fonts.googleapis.com/css2?family=Fraunces:ital,opsz,wght@0,9..144,400..900;1,9..144,400..900&family=Plus+Jakarta+Sans:wght@400;500;600;700&display=swap"
    rel="stylesheet"
  />
</head>
```

- [ ] **Step 2: Verify fonts load in dev**

Run in another terminal: `cd ramekin-ui && npm run dev` (or `make dev-ui` if that target exists — check the Makefile first).
Open the dev URL from `dev.env`. Open DevTools → Network → filter "font". Expected: Fraunces and Plus Jakarta Sans `.woff2` requests return 200.

- [ ] **Step 3: Commit**

```bash
git add ramekin-ui/index.html
git commit -m "Load Fraunces + Plus Jakarta Sans from Google Fonts"
```

---

### Task 2: Replace `index.css` with the token root

**Files:**
- Modify: `ramekin-ui/src/index.css`

- [ ] **Step 1: Overwrite `index.css` with the full light-mode token root**

Replace the entire contents of `ramekin-ui/src/index.css` with:

```css
/* Design tokens. All App.css rules must consume tokens from here — no
   literal hex values outside this file. */
:root {
  /* Palette — warm light */
  --bg-0: #faf5ec;
  --surface-1: #ffffff;
  --surface-2: #fbf6ed;
  --surface-elevated: #fffdf7;
  --border-hairline: #eadfce;
  --border-subtle: #ddcfb8;

  --text-primary: #1d1612;
  --text-secondary: #3d342c;
  --text-muted: #6f6256;
  --text-faint: #a89c8e;
  --text-on-accent: #fffdf7;

  --accent: #b47a1c;
  --accent-hover: #c78728;
  --accent-press: #8f6014;
  --accent-wash: #f4e7cc;

  --danger: #9c3a2c;
  --danger-hover: #b04438;
  --danger-wash: #f2dbd5;

  --focus-ring: color-mix(in oklab, var(--accent) 55%, transparent);

  /* Shadows */
  --shadow-card: 0 1px 2px rgba(35, 22, 10, 0.04),
    0 1px 1px rgba(35, 22, 10, 0.03);
  --shadow-hover: 0 6px 18px rgba(35, 22, 10, 0.09);
  --shadow-modal: 0 20px 60px rgba(35, 22, 10, 0.18);

  /* Radii */
  --radius-sm: 6px;
  --radius-md: 10px;
  --radius-lg: 14px;
  --radius-pill: 999px;

  /* Type */
  --font-display: "Fraunces", Georgia, serif;
  --font-body:
    "Plus Jakarta Sans", system-ui, -apple-system, Segoe UI, Roboto,
    Helvetica, Arial, sans-serif;
  --font-mono:
    ui-monospace, SFMono-Regular, Menlo, "Liberation Mono", monospace;

  /* Motion */
  --ease-out-soft: cubic-bezier(0.2, 0.7, 0.1, 1);
  --dur-fast: 0.15s;
  --dur-med: 0.22s;
  --dur-slow: 0.6s;

  font-family: var(--font-body);
  font-weight: 400;
  line-height: 1.5;
  color-scheme: light;
  color: var(--text-primary);
  background-color: var(--bg-0);

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

a {
  font-weight: 500;
  color: var(--accent);
  text-decoration: inherit;
}

a:hover {
  color: var(--accent-hover);
}

body {
  margin: 0;
  min-width: 320px;
  min-height: 100vh;
  background: var(--bg-0);
  color: var(--text-primary);
}

h1,
h2,
h3,
h4,
h5,
h6 {
  font-family: var(--font-display);
  font-weight: 500;
  color: var(--text-primary);
  letter-spacing: -0.01em;
  margin: 0 0 0.6em;
}

h1 {
  font-size: clamp(2rem, 1.2rem + 2vw, 2.75rem);
  font-variation-settings: "opsz" 144;
  line-height: 1.1;
}

h2 {
  font-size: 1.75rem;
  font-variation-settings: "opsz" 96;
  line-height: 1.15;
}

h3 {
  font-size: 1.25rem;
  font-variation-settings: "opsz" 72;
  line-height: 1.2;
}

button {
  font-family: inherit;
  font-weight: 500;
  border-radius: var(--radius-md);
  border: 1px solid transparent;
  padding: 0.55rem 1rem;
  font-size: 1rem;
  cursor: pointer;
  background-color: var(--surface-1);
  color: var(--text-primary);
  transition:
    background-color var(--dur-med) ease,
    border-color var(--dur-med) ease,
    color var(--dur-med) ease;
}

button:focus-visible,
a:focus-visible,
input:focus-visible,
select:focus-visible,
textarea:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
```

The earlier `@media (prefers-color-scheme: light)` block is deleted outright — we are light-only.

- [ ] **Step 2: Reload the app; confirm the background turns cream**

Reload the dev URL. Expected: body background is `#faf5ec`. The old dark theme remains visible in app chrome because `App.css` is still full of hardcoded colors — that's expected until Task 4+.

- [ ] **Step 3: Run lint**

Run: `make lint`
Expected: green.

- [ ] **Step 4: Commit**

```bash
git add ramekin-ui/src/index.css
git commit -m "Replace index.css with warm-light token root"
```

---

### Task 3: Baseline the hex-literal punchlist

No code changes — just capture the starting count so each subsequent task can compare against it.

- [ ] **Step 1: Count hex literals currently in App.css**

Run: `grep -cE '#[0-9a-fA-F]{3,8}' ramekin-ui/src/App.css`
Write the number down (expected ~340 at plan start). Each later task should lower this count.

- [ ] **Step 2: Quick sanity check**

Run: `grep -nE '#[0-9a-fA-F]{3,8}' ramekin-ui/src/App.css | head -5`
Confirm the hits look like the colors you expect (e.g. `#1a1a1a`, `#646cff`). The `color-no-hex` lint rule is introduced at the end (Task 16) as the acceptance check.

No commit — this is a read-only baseline.

---

### Task 4: App.css — reset + base layout section

The top of `App.css` is a short "Layout" block (app-layout, app-header, app-title, app-nav, app-main, app-footer). Convert it first — small, visible, sets the pattern.

**Files:**
- Modify: `ramekin-ui/src/App.css` (lines ~1–110)

- [ ] **Step 1: Replace the "Layout" section**

Replace the block that runs from `/* Layout */` through and including `.app-footer { ... }` with:

```css
/* Reset & layout */
*,
*::before,
*::after {
  box-sizing: border-box;
}

.app-layout {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
}

.app-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 1rem;
  padding: 1rem 2rem;
  background: var(--bg-0);
  border-bottom: 1px solid var(--border-hairline);
  position: relative;
}

.app-title {
  font-family: var(--font-display);
  font-style: italic;
  font-size: 1.5rem;
  font-weight: 600;
  color: var(--text-primary);
  text-decoration: none;
  font-variation-settings: "opsz" 96;
}

.app-title:hover {
  color: var(--accent);
}

.app-nav {
  display: flex;
  gap: 1.5rem;
  align-items: center;
}

.app-nav a {
  color: var(--text-secondary);
  text-decoration: none;
}

.app-nav a:hover {
  color: var(--text-primary);
}

.app-nav a.active {
  color: var(--accent);
}

.nav-link-button {
  background: transparent;
  border: none;
  padding: 0;
  color: var(--text-secondary);
  font: inherit;
  cursor: pointer;
}

.nav-link-button:hover:not(:disabled) {
  color: var(--text-primary);
}

.nav-link-button:disabled {
  opacity: 0.6;
  cursor: default;
}

.mobile-nav-toggle {
  display: none;
  border: 1px solid var(--border-hairline);
  border-radius: var(--radius-sm);
  background: var(--surface-1);
  color: var(--text-primary);
  padding: 0.45rem 0.8rem;
  cursor: pointer;
  font-size: 0.9rem;
}

.mobile-nav-toggle:hover {
  background: var(--surface-2);
}

.app-main {
  flex: 1;
  padding: 2rem;
  max-width: 1200px;
  margin: 0 auto;
  width: 100%;
}

.app-main:has(.create-recipe-page),
.app-main:has(.edit-recipe-page) {
  max-width: 800px;
}

.app-main:has(.view-recipe-page) {
  max-width: 1000px;
}

.app-footer {
  text-align: center;
  padding: 0.5rem 1rem;
  font-size: 0.75rem;
  color: var(--text-faint);
  border-top: 1px solid var(--border-hairline);
}
```

`#409` may have introduced additional classes like `.app-nav-inline` or `.account-menu`. If so, keep them and rewrite them using the same tokens — scan the old block before deleting to be sure nothing is lost. (Do not restore old dark-colored rules.)

- [ ] **Step 2: Reload app; verify header reads light**

Expected: cream app background, white-ish cards, dark-brown text. Card bodies still look dark because the recipe-card rules haven't been converted yet.

- [ ] **Step 3: Commit**

```bash
git add ramekin-ui/src/App.css
git commit -m "App.css layout section: consume tokens, warm light palette"
```

---

### Task 5: App.css — buttons

**Files:**
- Modify: `ramekin-ui/src/App.css` (`/* Buttons */` block)

- [ ] **Step 1: Rewrite the buttons section**

Locate `/* Buttons */` in `App.css` and replace the entire `.btn*` and `.logout-button` block with:

```css
/* Buttons */
.btn {
  display: inline-block;
  padding: 0.55rem 1rem;
  border: 1px solid var(--border-hairline);
  border-radius: var(--radius-md);
  background: var(--surface-1);
  color: var(--text-primary);
  text-decoration: none;
  cursor: pointer;
  font-family: var(--font-body);
  font-size: 0.9rem;
  font-weight: 500;
  transition:
    background-color var(--dur-med) ease,
    border-color var(--dur-med) ease;
}

.btn:hover {
  background: var(--surface-2);
  border-color: var(--border-subtle);
}

.btn-primary {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--text-on-accent);
}

.btn-primary:hover {
  background: var(--accent-hover);
  border-color: var(--accent-hover);
}

.btn-danger {
  background: var(--danger);
  border-color: var(--danger);
  color: var(--text-on-accent);
}

.btn-danger:hover {
  background: var(--danger-hover);
  border-color: var(--danger-hover);
}

.btn-danger-outline {
  background: transparent;
  border-color: var(--danger);
  color: var(--danger);
}

.btn-danger-outline:hover {
  background: var(--danger-wash);
}

.btn-small {
  padding: 0.25rem 0.5rem;
  font-size: 0.8rem;
}

.btn-header {
  padding: 0.4rem 0.8rem;
  font-size: 0.85rem;
}

.logout-button {
  background: transparent;
  border: 1px solid var(--border-hairline);
  color: var(--text-secondary);
  padding: 0.4rem 0.8rem;
  border-radius: var(--radius-md);
  cursor: pointer;
}

.logout-button:hover {
  background: var(--surface-2);
  color: var(--text-primary);
}
```

- [ ] **Step 2: Reload; click a few buttons**

Expected: "New recipe" / primary buttons are saffron with warm-white text; secondary buttons are white with a hairline border; danger buttons are muted cherry.

- [ ] **Step 3: Commit**

```bash
git add ramekin-ui/src/App.css
git commit -m "App.css buttons: tokens + warm palette"
```

---

### Task 6: App.css — recipe cards + cookbook grid

This is the flagship of the redesign. Card, thumbnail, title, description, hover.

**Files:**
- Modify: `ramekin-ui/src/App.css` — the `.recipe-grid`, `.recipe-card`, `.recipe-card-thumbnail`, `.recipe-card-placeholder`, `.recipe-card-content`, `.recipe-card h3`, `.recipe-card .recipe-description`, `.recipe-card .recipe-tags`, `.recipe-card .recipe-date`, `.recipe-card-selectable`, and `.recipe-card-checkbox` rules (around lines 580–730). Also the `.bulk-toolbar` block.

- [ ] **Step 1: Rewrite the cards block**

Replace the block starting at `.recipe-grid` through `.recipe-card .recipe-date` with:

```css
.recipe-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 1.25rem;
}

.recipe-card {
  display: flex;
  flex-direction: column;
  background: var(--surface-1);
  border: 1px solid var(--border-hairline);
  border-radius: var(--radius-lg);
  text-decoration: none;
  color: inherit;
  transition:
    border-color var(--dur-med) ease,
    box-shadow var(--dur-med) ease;
  overflow: hidden;
  height: 100%;
  box-shadow: var(--shadow-card);
}

.recipe-card:hover {
  border-color: color-mix(in oklab, var(--accent) 55%, var(--border-hairline));
  box-shadow: var(--shadow-hover);
}

.recipe-card:hover .recipe-card-thumbnail img,
.recipe-card:hover .recipe-card-placeholder {
  transform: scale(1.06);
}

.recipe-card-selectable {
  position: relative;
  cursor: pointer;
}

.recipe-card-selectable.selected {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px var(--accent);
}

.recipe-card-checkbox {
  position: absolute;
  top: 0.5rem;
  left: 0.5rem;
  z-index: 2;
  width: 1.25rem;
  height: 1.25rem;
  cursor: pointer;
}

.recipe-card-thumbnail {
  width: 100%;
  aspect-ratio: 1;
  overflow: hidden;
}

.recipe-card-thumbnail img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
  transition: transform var(--dur-slow) var(--ease-out-soft);
}

.recipe-card-placeholder {
  width: 100%;
  aspect-ratio: 1;
  background: linear-gradient(
    135deg,
    var(--surface-2) 0%,
    var(--surface-1) 100%
  );
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-faint);
  font-size: 3rem;
  transition: transform var(--dur-slow) var(--ease-out-soft);
}

.recipe-card-content {
  padding: 0.9rem 1rem 1rem;
  display: flex;
  flex-direction: column;
  flex: 1;
}

.recipe-card h3 {
  font-family: var(--font-display);
  font-variation-settings: "opsz" 96;
  font-weight: 500;
  margin: 0 0 0.4rem;
  color: var(--text-primary);
  font-size: 1.1rem;
  line-height: 1.2;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.recipe-card .recipe-description {
  color: var(--text-muted);
  font-size: 0.82rem;
  margin: 0 0 0.65rem;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  line-height: 1.4;
}

.recipe-card .recipe-tags {
  margin: auto 0 0.5rem;
}

.recipe-card .recipe-date {
  color: var(--text-faint);
  font-size: 0.75rem;
  margin: 0;
}

@media (prefers-reduced-motion: reduce) {
  .recipe-card-thumbnail img,
  .recipe-card-placeholder {
    transition: none;
  }
  .recipe-card:hover .recipe-card-thumbnail img,
  .recipe-card:hover .recipe-card-placeholder {
    transform: none;
  }
}
```

Also update `.bulk-toolbar` to use tokens (same file, a few lines below):

```css
.bulk-toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.5rem;
  padding: 0.75rem 1rem;
  margin-bottom: 1rem;
  background: var(--surface-1);
  border: 1px solid var(--border-hairline);
  border-radius: var(--radius-lg);
}

.bulk-count {
  font-weight: 600;
  margin-right: auto;
}
```

- [ ] **Step 2: Reload cookbook; verify hover feel**

Hover a card: the photo should inset-zoom slowly, the frame stays put, border gains a warm saffron tint. Focus via keyboard (Tab): 2px solid saffron outline.

- [ ] **Step 3: Commit**

```bash
git add ramekin-ui/src/App.css
git commit -m "Recipe cards: tokens, warm palette, inset photo hover"
```

---

### Task 7: App.css — cookbook page chrome (hero, search, filter sidebar, filter pills, tags)

The merge introduced `#410` sidebar + `#409` header rework. Re-theme all of it to tokens.

**Files:**
- Modify: `ramekin-ui/src/App.css` — all selectors under `.cookbook-page`, `.cookbook-filters`, `.filter-*`, `.sidebar-*`, plus the standalone `.search`, `.filter-pill`, `.tag`, and `.tag-input*` rules.

- [ ] **Step 1: Audit current rules**

Run: `grep -n "cookbook\|filter\|sidebar\|tag-input" ramekin-ui/src/App.css | head -80`
Write down the line ranges for each logical block (cookbook-filters, sidebar, filter-pills, tag-input). Keeping notes prevents missing a block.

- [ ] **Step 2: Rewrite the cookbook-page / filter / sidebar blocks**

For every rule in those blocks: replace every hex literal with the closest token from the list below. Use this mapping:

| Old color                       | New token          |
| ------------------------------- | ------------------ |
| `#1a1a1a` / `#1a1614`           | `var(--surface-1)` |
| `#2a2a2a` / `#222` / `#242424`  | `var(--surface-2)` |
| `#333`                          | `var(--border-hairline)` |
| `#444` / `#555`                 | `var(--border-subtle)` |
| `#fff`                          | `var(--text-primary)` (body) or `var(--text-on-accent)` (on accent fills) |
| `#ccc` / `#e0e0e0`              | `var(--text-secondary)` |
| `#888` / `#999`                 | `var(--text-muted)` |
| `#666`                          | `var(--text-faint)` |
| `#646cff` / `#535bf2`           | `var(--accent)` / `var(--accent-hover)` |
| `#dc3545` / `#c82333`           | `var(--danger)` / `var(--danger-hover)` |

Update `.filter-pill`, `.filter-pill.active`, and any checkbox / radio-looking filters to match the mockup — active pill uses `--accent-wash` fill, `color-mix(in oklab, var(--accent) 45%, var(--border-hairline))` border, `--accent-press` text.

Use `--radius-pill` for pill-shaped elements, `--radius-md` for inputs/search, `--radius-lg` for card-like containers.

- [ ] **Step 3: Reload; verify cookbook page in full**

Cookbook page should look like the mockup from earlier (hero roman, pills warm, sidebar light, grid warm). Click through filters to make sure active state reads clearly.

- [ ] **Step 4: Commit**

```bash
git add ramekin-ui/src/App.css
git commit -m "Cookbook chrome: tokens for hero, sidebar, filters, pills"
```

---

### Task 8: App.css — tags / tag inputs / chips

Tags appear on cards, in filters, in the TagInput component, and on the tags page. Keep them consistent.

**Files:**
- Modify: `ramekin-ui/src/App.css` — all `.tag`, `.tag-input*`, `.tag-suggestion*`, `.tag-chip`, `.tag-color-*` rules.

- [ ] **Step 1: Rewrite the tag rules**

Base `.tag` becomes:

```css
.tag {
  display: inline-block;
  padding: 0.15rem 0.55rem;
  border-radius: var(--radius-pill);
  background: var(--surface-2);
  border: 1px solid var(--border-hairline);
  color: var(--text-secondary);
  font-size: 0.72rem;
  line-height: 1.5;
}

.tag-removable {
  padding-right: 0.25rem;
}

.tag-remove {
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 0 0.25rem;
  font-size: 0.9rem;
  line-height: 1;
}

.tag-remove:hover {
  color: var(--danger);
}
```

For `.tag-input*` autocomplete dropdown: white background, hairline border, 10px radius, hovered item gets `var(--accent-wash)` fill.

- [ ] **Step 2: Reload; open a recipe form with tags**

Type to trigger suggestions. Expected: cream pills, dark-brown text, subtle border; suggestion list opens in white with hairline border, hovered item is a light saffron wash.

- [ ] **Step 3: Commit**

```bash
git add ramekin-ui/src/App.css
git commit -m "Tags and tag input: tokens + warm palette"
```

---

### Task 9: App.css — forms (inputs, selects, textareas, labels, create/edit/login)

**Files:**
- Modify: `ramekin-ui/src/App.css` — all `input`, `select`, `textarea`, `.form-group`, `.auth-form`, `.login-page`, `.create-recipe-page`, `.edit-recipe-page`, `.recipe-form*` rules.

- [ ] **Step 1: Define shared input surface**

Replace existing bare `input`, `select`, `textarea` rules and `.form-group` with:

```css
input[type="text"],
input[type="email"],
input[type="password"],
input[type="search"],
input[type="number"],
input[type="url"],
select,
textarea {
  background: var(--surface-1);
  border: 1px solid var(--border-hairline);
  border-radius: var(--radius-md);
  padding: 0.55rem 0.85rem;
  color: var(--text-primary);
  font-family: var(--font-body);
  font-size: 0.95rem;
  width: 100%;
  box-shadow: var(--shadow-card);
  transition: border-color var(--dur-med) ease;
}

input::placeholder,
textarea::placeholder {
  color: var(--text-muted);
}

input:focus,
select:focus,
textarea:focus {
  border-color: var(--border-subtle);
  outline: none;
}

input:focus-visible,
select:focus-visible,
textarea:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
  border-color: var(--border-subtle);
}

.form-group {
  margin-bottom: 1rem;
}

.form-group label {
  display: block;
  margin-bottom: 0.35rem;
  color: var(--text-secondary);
  font-weight: 500;
  font-size: 0.9rem;
}

.form-hint {
  color: var(--text-muted);
  font-size: 0.8rem;
  margin-top: 0.25rem;
}

.form-error {
  color: var(--danger);
  font-size: 0.85rem;
  margin-top: 0.25rem;
}
```

- [ ] **Step 2: Update page-specific form blocks**

For `.login-page`, `.auth-form`, `.create-recipe-page`, `.edit-recipe-page`, `.recipe-form*`, `.recipe-form-section-title`, `.ingredient-row`, `.instruction-row`: sweep every hex literal using the mapping from Task 7.

The login card specifically:

```css
.login-page {
  max-width: 420px;
  margin: 4rem auto;
  padding: 2.5rem;
  text-align: center;
  background: var(--surface-1);
  border: 1px solid var(--border-hairline);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-card);
}

.login-page h1 {
  margin-bottom: 1.5rem;
  font-size: 2.25rem;
}
```

- [ ] **Step 3: Reload; exercise every form**

Visit `/login` (cream card), `/recipes/new`, edit an existing recipe, type into tag input. All inputs should be white surfaces, saffron focus ring, dark-brown text.

- [ ] **Step 4: Commit**

```bash
git add ramekin-ui/src/App.css
git commit -m "Forms: shared input surface + page-specific blocks use tokens"
```

---

### Task 10: App.css — modals + backdrops + dialog variants

**Files:**
- Modify: `ramekin-ui/src/App.css` — `.modal*`, `.modal-backdrop`, `.modal-header`, `.modal-body`, `.modal-footer`, `.confirm-dialog*`, `.version-compare-modal*`, `.enrich-preview-modal*`.

- [ ] **Step 1: Rewrite the base modal block**

```css
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(29, 22, 18, 0.48);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
  padding: 1rem;
}

.modal {
  background: var(--surface-elevated);
  border: 1px solid var(--border-hairline);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-modal);
  max-width: 560px;
  width: 100%;
  max-height: min(90vh, 780px);
  overflow: auto;
  color: var(--text-primary);
}

.modal-header {
  padding: 1.1rem 1.5rem;
  border-bottom: 1px solid var(--border-hairline);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.modal-header h2,
.modal-header h3 {
  margin: 0;
  font-family: var(--font-display);
  font-weight: 500;
  font-size: 1.35rem;
}

.modal-body {
  padding: 1.25rem 1.5rem;
}

.modal-footer {
  padding: 1rem 1.5rem;
  border-top: 1px solid var(--border-hairline);
  display: flex;
  gap: 0.5rem;
  justify-content: flex-end;
}

.modal-close {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 1.25rem;
  padding: 0.25rem;
}

.modal-close:hover {
  color: var(--text-primary);
}

.modal-danger .modal-header {
  background: var(--danger-wash);
  border-bottom-color: var(--border-hairline);
}
```

For `.confirm-dialog*`, `.version-compare-modal*`, `.enrich-preview-modal*`, `.pdf-export-modal*`: sweep hex literals → tokens using the Task 7 mapping. Don't change layout; only colors, borders, radii, and shadows.

- [ ] **Step 2: Reload; trigger each modal**

Delete a recipe (confirm dialog), open version history (compare modal), open Add to shopping list, click Export PDF. Each should have a warm off-white dialog on a dim warm-translucent backdrop.

- [ ] **Step 3: Commit**

```bash
git add ramekin-ui/src/App.css
git commit -m "Modals: tokens, warm palette, danger-header wash"
```

---

### Task 11: App.css — recipe view page

**Files:**
- Modify: `ramekin-ui/src/App.css` — every rule under `.view-recipe-page`, `.recipe-meta`, `.recipe-ingredients`, `.recipe-instructions`, `.ingredient-line`, `.instruction-step`, `.recipe-photos`, `.star-rating`, `.enrich-section`, `.recipe-source`.

- [ ] **Step 1: Audit + sweep**

Run: `grep -n "view-recipe\|recipe-meta\|recipe-ingredients\|recipe-instructions\|ingredient-line\|instruction-step" ramekin-ui/src/App.css | head -80`

Work through each hit. Replace hex literals using the Task 7 mapping. Title `h1` inside `.view-recipe-page` should use the display font at a large size:

```css
.view-recipe-page h1 {
  font-family: var(--font-display);
  font-variation-settings: "opsz" 144;
  font-size: clamp(2rem, 1.2rem + 2vw, 2.75rem);
  font-weight: 500;
  letter-spacing: -0.015em;
  margin: 0 0 0.5rem;
  color: var(--text-primary);
}
```

`.recipe-description` on the view page reads in Plus Jakarta Sans at 1rem, `--text-secondary`. Ingredient lines use `--text-primary`; steps use `--text-secondary` for body and `--text-primary` for leading numbers.

- [ ] **Step 2: Reload; open a recipe**

Expected: editorial layout — big serif title, cream surface, dark-brown body. Star rating uses saffron. Photos render edge-to-edge where they did before (don't change photo structure).

- [ ] **Step 3: Commit**

```bash
git add ramekin-ui/src/App.css
git commit -m "Recipe view page: serif title + token sweep"
```

---

### Task 12: App.css — shopping list, meal plan, tags page, import, capture

One sweep task covering the remaining page-level rules. Each is smaller than the cookbook or recipe view.

**Files:**
- Modify: `ramekin-ui/src/App.css` — `.shopping-list*`, `.meal-plan*`, `.tags-page*`, `.import-page*`, `.capture-page*`, and any `.pdf-export-modal*` not already touched in Task 10.

- [ ] **Step 1: Sweep each page block**

For each section: list selectors with `grep -n`, then replace hex literals with tokens via the Task 7 mapping. Headings use the display font; body uses Plus Jakarta Sans; interactive surfaces match the button/form patterns.

- [ ] **Step 2: Reload; click through each page**

Visit `/shopping-list`, `/meal-plan`, `/tags`, `/import`, the capture/bookmarklet page. Each should match the overall look — no residual dark cards, no residual indigo accents.

- [ ] **Step 3: Commit**

```bash
git add ramekin-ui/src/App.css
git commit -m "Remaining pages: shopping list, meal plan, tags, import, capture"
```

---

### Task 13: App.css — misc utilities (loading, errors, badges, misc)

Pick up stragglers: `.loading`, `.error-message`, `.success-message`, `.badge*`, `.version-source-badge`, `.star-rating`, photo thumbnails, diff viewer colors.

**Files:**
- Modify: `ramekin-ui/src/App.css`

- [ ] **Step 1: Sweep**

```css
.loading {
  color: var(--text-muted);
  font-style: italic;
}

.error-message {
  background: var(--danger-wash);
  border: 1px solid color-mix(in oklab, var(--danger) 45%, var(--border-hairline));
  color: var(--danger);
  padding: 0.65rem 0.85rem;
  border-radius: var(--radius-md);
  font-size: 0.88rem;
}

.success-message {
  background: var(--accent-wash);
  border: 1px solid color-mix(in oklab, var(--accent) 45%, var(--border-hairline));
  color: var(--accent-press);
  padding: 0.65rem 0.85rem;
  border-radius: var(--radius-md);
  font-size: 0.88rem;
}
```

For diff viewer: red additions/deletions use `var(--danger)` tones; added lines use `var(--accent-wash)` background. Badges: hairline border, `--surface-2` background, `--text-secondary`.

Run: `grep -n "#[0-9a-fA-F]\{3,8\}" ramekin-ui/src/App.css | head -30` — every remaining hit should be caught here or in the later sweep task.

- [ ] **Step 2: Commit**

```bash
git add ramekin-ui/src/App.css
git commit -m "App.css utilities + badges + messages use tokens"
```

---

### Task 14: Paper-grain texture

**Files:**
- Create: `ramekin-ui/src/assets/grain.png`
- Modify: `ramekin-ui/src/App.css`

- [ ] **Step 1: Generate the grain asset**

Write the PNG with a tiny script. Create and run:

```bash
python3 - <<'PY'
from pathlib import Path
import struct, zlib, random

random.seed(42)
size = 200
# Grayscale 8-bit, centered around 128, ±32. mix-blend-mode: multiply at 3%
# opacity makes this just a texture wash.
pixels = bytearray()
for y in range(size):
    pixels.append(0)  # PNG filter byte per row
    for x in range(size):
        v = max(96, min(160, 128 + random.randint(-32, 32)))
        pixels.append(v)

def chunk(tag, data):
    return (
        struct.pack(">I", len(data))
        + tag
        + data
        + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    )

ihdr = struct.pack(">IIBBBBB", size, size, 8, 0, 0, 0, 0)  # 8-bit grayscale
idat = zlib.compress(bytes(pixels), level=9)
png = (
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", ihdr)
    + chunk(b"IDAT", idat)
    + chunk(b"IEND", b"")
)

out = Path("ramekin-ui/src/assets/grain.png")
out.parent.mkdir(parents=True, exist_ok=True)
out.write_bytes(png)
print(f"wrote {out} ({len(png)} bytes)")
PY
```

Expected: file size under 10 KB. Inspect it visually — should look like subtle neutral noise, not a recognizable pattern.

- [ ] **Step 2: Add the overlay in App.css**

Append to `App.css` (under a new `/* Texture */` section heading, above the media queries block):

```css
/* Texture */
@media (prefers-reduced-motion: no-preference) {
  body::before {
    content: "";
    position: fixed;
    inset: 0;
    background-image: url("./assets/grain.png");
    background-size: 200px 200px;
    opacity: 0.03;
    mix-blend-mode: multiply;
    pointer-events: none;
    z-index: 0;
  }

  /* Keep real content above the grain */
  .app-layout,
  .modal-backdrop {
    position: relative;
    z-index: 1;
  }
}
```

Vite resolves the `./assets/grain.png` relative to `App.css`.

- [ ] **Step 3: Reload; verify grain visible but subtle**

Expected: faint paper texture over the cream background. Toggle macOS System Settings → Accessibility → Display → Reduce Motion; reload; expected: grain disappears.

- [ ] **Step 4: Commit**

```bash
git add ramekin-ui/src/assets/grain.png ramekin-ui/src/App.css
git commit -m "Paper-grain texture overlay (reduced-motion gated)"
```

---

### Task 15: Consolidate media queries to `--bp-*` breakpoints

**Files:**
- Modify: `ramekin-ui/src/App.css`

- [ ] **Step 1: Audit current breakpoints**

Run: `grep -n "@media" ramekin-ui/src/App.css`
Note the widths currently used (480, 600, 700, 768, 900, plus whatever the merge added).

- [ ] **Step 2: Collapse to three named values**

Define at the very top of `App.css`, above the first rule, a comment block documenting the three names:

```css
/* Breakpoints (keep in sync with the design doc):
   --bp-sm: 640px   small → medium
   --bp-md: 960px   medium → large
   --bp-lg: 1280px  large → xl
   Use only these three min-width values across the file. Any exception must
   include an inline comment explaining why. */
```

Then pass through every `@media (min-width: ...)` or `@media (max-width: ...)` rule and snap to the nearest named breakpoint:

- ≤600 → `max-width: 640px` (i.e. below `--bp-sm`)
- 601–900 → `max-width: 960px`
- ≥768 → `min-width: 960px`

Edge cases: orientation queries, `max-height` queries, and `prefers-reduced-motion` queries are left alone.

- [ ] **Step 3: Click-test on narrow + wide viewports**

In DevTools, test 375px, 768px, 1024px, 1600px. Layouts shouldn't visibly shift in the wrong place.

- [ ] **Step 4: Commit**

```bash
git add ramekin-ui/src/App.css
git commit -m "Consolidate App.css breakpoints to 640/960/1280"
```

---

### Task 16: Final hex sweep + enforce the tokens-only rule

**Files:**
- Modify: `ramekin-ui/src/App.css` (only if any hex literals remain)
- Modify: `ramekin-ui/.stylelintrc.json` (add the permanent rule)

- [ ] **Step 1: Final audit**

Run: `grep -nE '#[0-9a-fA-F]{3,8}' ramekin-ui/src/App.css`
Expected: zero matches. If any remain, substitute the closest token per the Task 7 mapping. `rgba(...)` values are fine and shouldn't be flagged.

- [ ] **Step 2: Add the permanent stylelint rule**

Overwrite `ramekin-ui/.stylelintrc.json` with:

```json
{
  "extends": ["stylelint-config-standard"],
  "rules": {
    "selector-class-pattern": "^[a-z][a-z0-9]*(-[a-z0-9]+)*$",
    "no-descending-specificity": null,
    "color-function-notation": null,
    "alpha-value-notation": null,
    "color-function-alias-notation": null,
    "media-feature-range-notation": null,
    "comment-empty-line-before": null,
    "declaration-empty-line-before": null,
    "value-keyword-case": null,
    "rule-empty-line-before": null,
    "color-hex-length": null
  },
  "overrides": [
    {
      "files": ["src/App.css"],
      "rules": {
        "color-no-hex": [
          true,
          {
            "message": "Use a design token from index.css instead of a hex literal"
          }
        ]
      }
    }
  ]
}
```

- [ ] **Step 3: Run full lint**

Run: `make lint`
Expected: all green. If CSS lint fails, fix the offending literal and re-run.

- [ ] **Step 4: Commit**

```bash
git add ramekin-ui/src/App.css ramekin-ui/.stylelintrc.json
git commit -m "Enforce tokens-only rule on App.css"
```

---

### Task 17: Accessibility + contrast verification

**Files:**
- Modify: `ramekin-ui/src/index.css` (only if adjustments needed)

- [ ] **Step 1: Run contrast checks on every text/surface pair**

Use the tiny script below (copy-paste into a Node REPL or save as `scripts/check-contrast.js` one-off; no need to commit the script):

```js
const pairs = [
  ["text-primary", "#1d1612", "bg-0", "#faf5ec"],
  ["text-secondary", "#3d342c", "bg-0", "#faf5ec"],
  ["text-muted", "#6f6256", "bg-0", "#faf5ec"],
  ["text-muted", "#6f6256", "surface-1", "#ffffff"],
  ["text-primary", "#1d1612", "surface-1", "#ffffff"],
  ["text-on-accent", "#fffdf7", "accent", "#b47a1c"],
  ["text-on-accent", "#fffdf7", "danger", "#9c3a2c"],
  ["accent-press", "#8f6014", "accent-wash", "#f4e7cc"],
];
const toRgb = (hex) => {
  const n = parseInt(hex.slice(1), 16);
  return [n >> 16, (n >> 8) & 0xff, n & 0xff];
};
const luma = (rgb) => {
  const ch = (c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * ch(rgb[0]) + 0.7152 * ch(rgb[1]) + 0.0722 * ch(rgb[2]);
};
const ratio = (a, b) => {
  const [la, lb] = [luma(toRgb(a)), luma(toRgb(b))].sort((x, y) => y - x);
  return (la + 0.05) / (lb + 0.05);
};
for (const [a, aHex, b, bHex] of pairs) {
  console.log(`${a} on ${b}: ${ratio(aHex, bHex).toFixed(2)}`);
}
```

Run with `node` (the one in `ramekin-ui/node_modules/.bin/` or system node is fine).

- [ ] **Step 2: Fix anything under 4.5:1 for body text or 3:1 for large/headings**

If a pair fails, darken or lighten the weaker token in `index.css` until it passes; update the design doc's token list to match.

- [ ] **Step 3: Keyboard smoke-test**

In the browser, Tab through the cookbook page, filter sidebar, a recipe card, a modal open/close, a form. Confirm every focused element has the 2px saffron outline and is visible.

- [ ] **Step 4: Reduced-motion smoke-test**

Enable Reduce Motion (macOS System Settings → Accessibility → Display). Reload. Confirm grain disappears and card photos don't scale on hover.

- [ ] **Step 5: Commit any token tweaks**

```bash
git add ramekin-ui/src/index.css docs/agent/2026-04-17-warmer-cookbook-aesthetic-design.md
git commit -m "Tune tokens for WCAG AA contrast"
```

Skip this commit if nothing changed.

---

### Task 18: Manual browser verification pass

No code changes — just eyes on every page.

- [ ] **Step 1: Start the dev server**

Run: `make dev-ui` (or `cd ramekin-ui && npm run dev`).

- [ ] **Step 2: Walk through each page and note screenshots**

Visit:

- `/login`
- `/cookbook` (default user with recipes)
- `/recipes/<id>` (view)
- `/recipes/<id>/edit`
- `/recipes/new`
- `/shopping-list`
- `/meal-plan`
- `/tags`
- `/import`
- `/capture` or the bookmarklet flow

Use the `audit-ui` skill if the dev server setup isn't familiar. On each page check:

1. No dark surfaces remaining.
2. No indigo / purple accents.
3. Fonts: Fraunces for headings, Plus Jakarta Sans for body.
4. Saffron CTA, muted cherry for destructive.
5. Focus ring is a saffron outline.

- [ ] **Step 3: Run the Playwright smoke suite**

Run: `make test`
Expected: everything green. If a smoke test was asserting on a hex color or dark-mode attribute, update it — but prefer keeping the assertions structural.

- [ ] **Step 4: Commit any smoke-test updates**

```bash
git add tests/ui/test_smoke.py
git commit -m "Update smoke assertions for light theme"
```

Skip this commit if nothing changed.

---

### Task 19: Delete the issue file + simplify pass

- [ ] **Step 1: Run the repo's simplify skill**

If the repo has a `/simplify` skill, run it. Otherwise scan the diff: any duplicated blocks between Task 4–13, any orphan selectors (rules with no matching markup), any comments that describe the old dark palette. Remove what you find.

- [ ] **Step 2: Delete the issue file**

```bash
rm issues/p3-warmer-cookbook-aesthetic.json5
git add issues/p3-warmer-cookbook-aesthetic.json5
```

- [ ] **Step 3: Final `make lint` + `make test`**

Run: `make lint && make test`
Expected: all green.

- [ ] **Step 4: Commit**

```bash
git commit -m "Ship warmer cookbook aesthetic + drop issue file"
```

---

### Task 20: Push + finalize PR

- [ ] **Step 1: Push**

Run: `git push origin HEAD`

- [ ] **Step 2: Finalize PR**

Run:

```bash
issue-finalize-pr \
  --title "Warmer cookbook aesthetic: light editorial palette + design tokens" \
  --body "$(cat <<'EOF'
## Issue context

`p3-warmer-cookbook-aesthetic` called out that the app reads as
"Vite starter dark theme": cool near-blacks, indigo accent, generic
sans everywhere. Nothing about the UI said "cookbook."

## What this PR does

- Introduces design tokens in `index.css` (surfaces, text, accent,
  danger, shadows, radii, fonts, motion). `App.css` consumes tokens
  only; a stylelint override forbids hex literals in `App.css` so we
  don't regress.
- Swaps to a warm light editorial palette (cream app bg, saffron CTA,
  muted cherry for destructive).
- Typography: Fraunces (variable serif) for display + Plus Jakarta Sans
  for body, loaded from Google Fonts with preconnect hints.
- Cards get a calm inset photo hover and a 2px saffron focus ring.
- 3% paper-grain body overlay, gated on `prefers-reduced-motion`.
- Applied everywhere — cookbook, recipe view, create/edit, login,
  shopping list, meal plan, tags, import/capture, all modals. Light-only;
  the old dark-mode styling and `prefers-color-scheme: light` media block
  are removed.
- Consolidates App.css media queries to three breakpoints: 640/960/1280.
- PDF export left alone (greyscale jsPDF print output).

## Test plan

- [ ] `make lint` — clean, including the new `color-no-hex` rule on
  `App.css`.
- [ ] `make test` — Playwright smoke green.
- [ ] Manual pass: cookbook, recipe view, create/edit, login, modals,
  shopping list, meal plan, tags, import/capture.
- [ ] Keyboard focus: every interactive element shows a saffron outline.
- [ ] Reduce Motion: grain hidden and card photos don't scale.
- [ ] Contrast: every text/surface pair meets WCAG AA at its body size.

Design spec: `docs/agent/2026-04-17-warmer-cookbook-aesthetic-design.md`
Implementation plan: `docs/agent/plans/2026-04-17-warmer-cookbook-aesthetic.md`
EOF
)"
```

- [ ] **Step 3: Watch CI**

Run: `issue-watch-pr`

Follow the watcher loop per the `solve-issue` skill: address any CI or codex review feedback, commit fixes, push, re-run the watcher. Cap at 10 fix iterations.

---

## Self-Review Checklist (run after finishing the plan)

- [ ] **Spec coverage:** every section of the design doc maps to at least one task above.
- [ ] **Placeholder scan:** no TBD, no "similar to Task N", no "handle edge cases", no undefined references.
- [ ] **Type consistency:** token names used in tasks match the exact names defined in Task 2 (`--bg-0`, not `--surface-0`).
- [ ] **No raw SQL, no `#noqa`, no backwards-compat cruft** — this is a CSS-only PR, but still worth eyeballing.
