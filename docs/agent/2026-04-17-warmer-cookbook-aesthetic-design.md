# Warmer cookbook aesthetic — design

Spec for issue `p3-warmer-cookbook-aesthetic`. Shifts the app from the generic
"Vite starter dark theme" look to a warm editorial cookbook feel, applied
across every page. Deliberately not design-only: this PR also introduces the
design-token foundation called out in `blocked-css-cleanup`.

## Goals

- The app visually reads as "cookbook," not "dashboard." Warm surfaces, a
  characterful display serif, intentional accent, calm interactions.
- CSS tokens replace the ~340 hardcoded hex values in `App.css`, centralizing
  surface/border/text/accent colors, fonts, radii, and breakpoints.
- The redesign is applied everywhere in the app in one PR (cookbook,
  recipe view, create/edit, login, modals, misc pages) so the change reads as
  one intentional moment rather than drift.

## Non-goals

- **No dark mode.** The app ships light-only. The previous dark-only styling
  and the unused `@media (prefers-color-scheme: light)` block in `index.css`
  both go away. If dark mode comes back, that's a separate future project.
- **PDF card export stays as-is.** `PdfExportModal.tsx` draws directly with
  jsPDF in greyscale; it's a print artifact and not bound to the web theme.
- **No layout changes.** The cookbook grid, sidebar, and nav shipped in the
  preceding PRs; this is tokens + palette + typography + hover, not new
  structure.

## Tokens

All tokens live at the top of `index.css` on `:root`. App.css consumes them
and contains no literal color values post-refactor.

### Palette (light, warm)

```
--bg-0:              #faf5ec   cream app background
--surface-1:         #ffffff   card / input surface
--surface-2:         #fbf6ed   recessed tile / tag pill / placeholder
--surface-elevated:  #fffdf7   hovered / modal dialog
--border-hairline:   #eadfce   default 1px border
--border-subtle:     #ddcfb8   stronger contextual border
--text-primary:      #1d1612   body text & headings
--text-secondary:    #3d342c   nav links, secondary body
--text-muted:        #6f6256   metadata, descriptions
--text-faint:        #8e8074   timestamps, counts
--accent:            #9a611a   saffron, darkened for AA on light bg
--accent-hover:      #8a5614   hover goes darker (not lighter) for AA
--accent-press:      #6e4310
--accent-wash:       #f4e7cc   selected filter pill fill
--danger:            #9c3a2c   muted cherry — destructive actions
--danger-hover:      #b04438
--danger-wash:       #f2dbd5   destructive dialog backdrop
--focus-ring:        color-mix(in oklab, var(--accent) 55%, transparent)
--shadow-card:       0 1px 2px rgba(35, 22, 10, 0.04),
                     0 1px 1px rgba(35, 22, 10, 0.03)
--shadow-hover:      0 6px 18px rgba(35, 22, 10, 0.09)
--shadow-modal:      0 20px 60px rgba(35, 22, 10, 0.18)
```

The accent is darkened from the issue's `#e8a33d` suggestion (which was
tuned for dark surfaces) to keep AA contrast on a cream background for both
white-on-accent CTA text and accent-on-cream states.

### Typography

```
--font-display: 'Fraunces', Georgia, serif;          /* variable, opsz 9..144 */
--font-body:    'Plus Jakarta Sans', system-ui, sans-serif;
--font-mono:    ui-monospace, SFMono-Regular, Menlo, monospace;
```

- Fraunces at `opsz 96` for card h3 and section headings, `opsz 144` for page
  hero headings, italic reserved for brand + "View →" micro-copy.
- Plus Jakarta Sans for body, nav, buttons, labels.
- Both loaded from Google Fonts via `<link>` in `index.html` with
  `preconnect` hints; the Fraunces variable axes in use are `opsz` (opsz)
  and `ital`.

### Spacing, radii, breakpoints

```
--radius-sm:  6px        /* inputs, small buttons */
--radius-md:  10px       /* standard button, search input */
--radius-lg:  14px       /* cards, modals */
--radius-pill: 999px

--bp-sm:  640px
--bp-md:  960px
--bp-lg:  1280px
```

The old App.css uses 480/600/700/768/900 media queries inconsistently. This
PR collapses them to `--bp-sm/md/lg` used in value-only (`@media (min-width:
640px)` etc.), with an exceptions list documented inline where genuinely
needed (e.g. `max-height` for the auth card).

## Component changes

### Cards (`recipe-card`, cookbook tile)

- Background `--surface-1`, 1px `--border-hairline`, `--radius-lg` corners.
- Photo bleeds to card edges at the top (no inner padding around the
  thumbnail); overflow-hidden on the card clips the image to match the
  corner radius.
- Hover: border shifts to `color-mix(in oklab, var(--accent) 55%,
  var(--border-hairline))`, card takes `--shadow-hover`. The photo inside
  scales to `1.06` over 0.6s `cubic-bezier(0.2, 0.7, 0.1, 1)` — the card
  frame stays put. This replaces today's `translateY(-4px)` + bright
  indigo shadow.
- Focus-visible: 2px solid `--accent` outline + 2px offset, bypassing the
  hover-only transitions so keyboard use is unambiguous.
- `prefers-reduced-motion: reduce` cancels the photo scale transition.

### Buttons

- Primary: `--accent` fill, `#fffdf7` text (warm near-white), no border.
- Secondary: `--surface-1` fill, `--border-hairline`, `--text-primary` text.
- Destructive: `--danger` fill, `#fffdf7` text, `--danger-hover` on hover.
  `.btn-danger-outline` uses `--danger` border + text, `--danger-wash` hover
  fill.
- All buttons 10px radius, 0.55rem 1rem padding, Plus Jakarta Sans 600.
- `button:focus-visible` gets a 2px accent outline with 2px offset.

### Header & nav

The structure already landed in `#409 Tighten header nav`. This PR just:

- Brand wordmark uses Fraunces italic.
- Nav links use `--text-secondary`, hover `--text-primary`, active
  `--accent`.
- Mobile hamburger uses the button-secondary token set.

### Filters sidebar

Shipped in `#410`. Re-themed: sidebar surface `--surface-1`, hairline border
right, section titles in `--text-secondary` + label letter-spacing, pill
filters use the shared pill recipe (see toolbar).

### Forms (create, edit, login, settings)

- Inputs: `--surface-1` background, `--border-hairline` border, `--radius-md`,
  `--text-primary` text, `--text-muted` placeholder. Focus adds
  `--border-subtle` border + the focus ring shadow.
- Labels: Plus Jakarta Sans 500, `--text-secondary`.
- Login card keeps its centered layout; re-themed only.

### Modals

- Backdrop `rgba(29, 22, 18, 0.48)`.
- Dialog background `--surface-elevated`, `--radius-lg`, `--shadow-modal`.
- Destructive-confirm modals use a `--danger-wash` header strip so the
  severity reads at a glance.

### Misc pages (Shopping list, Meal plan, Tags, Import, Capture)

Audited as part of the refactor — they all pull from the same shared token
set. No bespoke palettes, no remaining hex literals. Any page-local look
(e.g. tag pill) becomes a reusable class in a dedicated section of the
refactored App.css.

## Paper-grain texture

A `::before` pseudo-element on `body` carries a tileable PNG at 3% opacity,
`mix-blend-mode: multiply`, `pointer-events: none`, and `position: fixed`
covering the viewport so the grain doesn't scroll with content. `z-index`
sits below all interactive layers (modals, nav, dropdowns). Gated by
`@media (prefers-reduced-motion: no-preference)` so users who've opted out
of decoration get flat surfaces.

The grain image is a 200×200 8-bit tileable PNG checked into
`ramekin-ui/src/assets/grain.png` (target ≤10 KB). Generated once; no build
step dependency.

## Interaction details

- Nav / link transitions: 0.15s ease for color.
- Button state transitions: 0.18s ease for background + border.
- Card hover image scale: 0.6s `cubic-bezier(0.2, 0.7, 0.1, 1)` — noticeably
  slower than today's 0.2s transform, because it's a subtler motion.
- `prefers-reduced-motion: reduce` zeroes all transition durations except
  focus-ring (which remains instant and visible).

## Accessibility

- Implementation must verify every text-on-surface token pair against WCAG
  AA (4.5:1 for body, 3:1 for headings/large). If `--text-muted` on `--bg-0`
  or `#fffdf7` on `--accent` falls short, darken/lighten until it passes;
  these tokens are the intent, not a promise.
- Keyboard focus is always a 2px solid accent outline with offset, never
  replaced by a hover-only treatment.
- `prefers-reduced-motion: reduce` cancels photo scale + texture.

## CSS structure

App.css is reorganized top-down:

```
1. Reset & base
2. Tokens (fallback block; :root lives in index.css)
3. Typography base (h1-h6, body, a)
4. Buttons
5. Inputs / forms
6. Tags, pills
7. Cards (recipe-card, generic)
8. Header & nav
9. Cookbook layout (sidebar + grid)
10. Recipe view
11. Create / Edit forms
12. Modals
13. Shopping list
14. Meal plan
15. Import / Capture / Tags pages
16. Utilities (.loading, etc.)
17. Media queries — consolidated at the end per section using
    --bp-* custom media names
```

No file split on this pass (the issue explicitly says co-location is
optional, and splitting multiplies review surface). A future follow-up may
extract per-page stylesheets once the single-file version is stable.

## Testing

- Existing Playwright smoke tests (`tests/ui/test_smoke.py`) should pass
  unchanged; they assert on text + structure, not colors.
- `make lint` must stay green. The CSS linter in particular will flag
  remaining hex literals in App.css — the acceptance bar is literally "no
  hex literals outside the token block."
- Manual verification in browser: cookbook grid, recipe view, create/edit
  flow, login, modals. Screenshot comparison against the mockups.
- `prefers-reduced-motion` + keyboard-only navigation smoke-tested
  manually.

## Migration

- `index.html`: add Google Fonts `<link>` + preconnect.
- `index.css`: replace existing `:root` and media block with the new token
  root + typography base.
- `App.css`: search-and-replace pass across all hex literals → token; audit
  for duplicates; consolidate media queries.
- Remove any now-unused selectors the merge surfaced (e.g. old `.app-header`
  variants superseded by `#409`).

All in one PR. The issue is explicit: "Ship this as one PR so it reads as
an intentional redesign, not a drift."

## Out of scope / follow-ups

- `blocked-css-cleanup` unblocks after this PR. Its scope narrows to: file
  split, consolidate remaining breakpoints, remove dead selectors, introduce
  `clamp()` utilities. Palette and typography will already be tokens.
- `p2-cookbook-density-toggle` remains open and unaffected.
