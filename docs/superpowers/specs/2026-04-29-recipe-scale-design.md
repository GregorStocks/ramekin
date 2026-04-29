# Recipe ingredient scaling — design

## Goal

When viewing a recipe, let the user multiply ingredient amounts by a chosen factor (whole-number multipliers like `2×`, `3×`; unit-fraction divisors like `½×`, `⅓×`; or any custom decimal). Purely client-side; no API/backend changes.

## Scope

In scope:

- Scale ingredient `amount` strings on the recipe view page.
- Scale alternative measurements alongside the primary (each measurement scales independently through the same function).
- Scale the displayed `Serves:` value when it's a parseable number.
- Carry the active scale into the "Add to Shopping List" flow so the shopping list reflects the scaled amounts.

Out of scope:

- No persistence to DB, no new API endpoints, no version-history entries.
- No effect on enrichment, edit page, version compare, PDF export, or other surfaces.
- No range support — `"6-8"` (and similar ranged amounts) are left untouched.
- No scaling of free-text amounts like `"a pinch"`, `"to taste"`, `"about 2"`.
- Instructions text is never scaled (no temperature/time changes).

## UX

A "Scale" strip sits directly above the `Ingredients` heading inside the left column of the recipe view:

```
Ingredients   (scaled 2×)
─────────────────────────
Scale:  [¼×] [½×] [1×] [2×] [3×]   Custom: [____]×
```

- Preset buttons: `¼×`, `½×`, `1×`, `2×`, `3×`. The active one is visually highlighted.
- Custom input: a small `<input type="number" step="0.25" min="0">` next to the presets. Pressing Enter or blurring applies the value. Picking a preset clears any custom value, and entering a custom value de-highlights all presets (unless it matches one).
- When `scale ≠ 1`, a subtle `(scaled 2×)` badge appears next to the `Ingredients` heading and next to the `Serves:` line. Clicking `1×` resets.
- Invalid input (`0`, negative, `NaN`, blank) is ignored and the scale stays at its previous value.

State lives in the URL as `?scale=2` (or `?scale=0.5`, etc.). Refresh and link-sharing preserve it. Default (omitted/`1`) renders as the canonical recipe.

## Architecture & data flow

Pure client-side feature. New module `ramekin-ui/src/utils/scaleAmount.ts` exposes:

```ts
function scaleAmount(amount: string | null | undefined, factor: number): string
```

`ViewRecipePage`:

- Reads `scale` from `searchParams` (default `1`). Provides setters that write back to `searchParams`.
- When rendering each ingredient measurement, replaces the raw `amount` with `scaleAmount(amount, scale)`.
- Replaces the `Serves:` value with `scaleAmount(servings, scale)` when `servings` parses; otherwise renders it unchanged.
- Passes `scale` to `AddToShoppingListModal` as a new prop.

`AddToShoppingListModal`:

- Accepts an optional `scale` prop (default `1`).
- Before posting items to the shopping-list API, runs each ingredient measurement's amount through `scaleAmount`.

No other components change.

## Parsing & formatting (`scaleAmount`)

**Parser** accepts:

- integer (`"2"`),
- decimal (`"2.5"`),
- vulgar fraction (`"1/2"`, also Unicode `½`, `⅓`, `¼`, `⅔`, `¾`, `⅛`, `⅜`, `⅝`, `⅞`),
- mixed number (`"1 1/2"`, `"1½"`).

Anything else — including ranges (`"6-8"`, `"6 to 8"`), free text (`"a pinch"`, `"to taste"`), empty strings, and amounts with surrounding words (`"about 2"`) — is recognized as unparseable; the original string is returned unchanged regardless of factor.

**Formatter** picks output shape from the *result* value (not the input shape):

- If the result is an integer (within a small float tolerance), render as `"3"`.
- Else if the result equals `1/N` for `N ∈ {2, 3, 4, 6, 8}` (within tolerance), render as `"1/N"`.
- Otherwise render as a decimal trimmed to at most 2 places with trailing zeros dropped (`"1.5"`, `"0.33"`).

Rationale: per the agreed rule, output is whole numbers, unit-fraction divisors, or decimals — no mixed numbers, and non-unit fractions like `2/3` collapse to decimals (`"0.67"`). Predictable and easy to skim.

**Edge cases the formatter handles:**

- `factor = 1` is a no-op (return original string).
- `factor ≤ 0` or `NaN` — `scaleAmount` returns the original string and the UI ignores the value.
- Floating-point fuzz (e.g., `1/3 × 3 = 0.999…`) is snapped to integer / unit fraction by tolerance check (`abs(x - round) < 1e-6`).

## Files touched

- `ramekin-ui/src/utils/scaleAmount.ts` — new module (parser + formatter + tests).
- `ramekin-ui/src/utils/scaleAmount.test.ts` — unit tests (Vitest, matching existing UI test conventions).
- `ramekin-ui/src/pages/ViewRecipePage.tsx` — scale signal, URL plumbing, `<ScaleControls>` strip above ingredients, scaled rendering for amounts and `Serves:`, scale badge.
- `ramekin-ui/src/components/AddToShoppingListModal.tsx` — accept `scale` prop, run amounts through `scaleAmount` before posting.
- `ramekin-ui/src/App.css` — styles for `.scale-controls`, `.scale-preset`, `.scale-preset.active`, `.scale-badge`.

No Rust, no DB migrations, no OpenAPI spec change.

## Testing

- **Unit**: `scaleAmount.test.ts` covers integer × integer, fraction × integer, mixed × fraction, decimal × decimal, Unicode fractions, floating-point fuzz, range strings (passed through), free-text strings (passed through), `factor ≤ 0` / `NaN` (passed through), empty string.
- **UI (Playwright)**: extend `tests/ui/` with a recipe-scale test that:
  1. Loads a recipe with known ingredients.
  2. Clicks `2×` and asserts amounts doubled and the `(scaled 2×)` badge appears.
  3. Clicks `1×` and asserts amounts revert.
  4. Enters a custom `0.5` and asserts amounts halve.
  5. Verifies the URL contains `?scale=2` after clicking `2×`.
- **UI (Playwright)**: extend the existing shopping-list test (or add one) to scale a recipe to `2×`, add to shopping list, and assert the posted amounts are doubled.

## Open questions

None at design time.
