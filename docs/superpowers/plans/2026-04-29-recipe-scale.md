# Recipe Ingredient Scaling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a client-only "Scale" control to the recipe view that multiplies ingredient amounts (and parseable Serves: values) by a chosen factor, with the active scale flowing through to the "Add to Shopping List" flow.

**Architecture:** A pure-function utility (`scaleAmount`) parses a recipe's amount string, multiplies by a factor, and re-formats. The view page stores the scale in URL search params (`?scale=N`) and pipes it through ingredient rendering, the `Serves:` line, and the existing `AddToShoppingListModal`. No backend, API, schema, or generated-client changes.

**Tech Stack:** SolidJS UI (Solid signals + `@solidjs/router` `useSearchParams`), TypeScript, Python Playwright tests via `tests/ui/`.

**Spec:** `docs/superpowers/specs/2026-04-29-recipe-scale-design.md`

---

## File Structure

**Create:**
- `ramekin-ui/src/utils/scaleAmount.ts` — parser + formatter, exports `scaleAmount(amount, factor)`.
- `tests/ui/test_recipe_scale.py` — Playwright coverage for the controls, ingredient/Serves scaling, shopping-list integration.

**Modify:**
- `ramekin-ui/src/pages/ViewRecipePage.tsx` — add scale signal/URL plumbing, render `<ScaleControls>` strip, route `amount` and `servings` through `scaleAmount`, show `(scaled N×)` badge.
- `ramekin-ui/src/components/AddToShoppingListModal.tsx` — accept `scale` prop, scale amounts in both the in-modal display and the API payload.
- `ramekin-ui/src/App.css` — styles for `.scale-controls`, `.scale-preset`, `.scale-preset.active`, `.scale-custom-input`, `.scale-badge`.

No Rust, no DB migrations, no OpenAPI / generated-client regeneration.

---

## Conventions for this plan

- Run **`make lint`** to typecheck/format-check after any TS/CSS change.
- Run **`make test-ui`** to execute the Playwright suite. There is no per-test filter wired through the Makefile, so the whole UI suite runs each time (~minutes). The new test file is `tests/ui/test_recipe_scale.py` — look for its tests in the pytest output.
- Each task ends with **one commit**. Use the `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>` trailer the project uses elsewhere.

---

## Task 1: Add the `scaleAmount` utility module

A pure function that knows how to parse, multiply, and re-format a single amount string. This task lands the utility in isolation; later tasks consume it.

**Files:**
- Create: `ramekin-ui/src/utils/scaleAmount.ts`

- [ ] **Step 1: Write the module**

Create `ramekin-ui/src/utils/scaleAmount.ts` with this exact content:

```ts
const UNICODE_FRACTIONS: Record<string, string> = {
  "½": "1/2",
  "⅓": "1/3",
  "⅔": "2/3",
  "¼": "1/4",
  "¾": "3/4",
  "⅕": "1/5",
  "⅖": "2/5",
  "⅗": "3/5",
  "⅘": "4/5",
  "⅙": "1/6",
  "⅚": "5/6",
  "⅛": "1/8",
  "⅜": "3/8",
  "⅝": "5/8",
  "⅞": "7/8",
};

const UNIT_FRACTION_DENOMS = [2, 3, 4, 6, 8] as const;
const FLOAT_TOL = 1e-6;

function normalizeFractions(input: string): string {
  let out = "";
  for (const ch of input) {
    if (ch in UNICODE_FRACTIONS) {
      // Insert a space before a Unicode fraction so "1½" parses as "1 1/2"
      if (out.length > 0 && /[0-9]$/.test(out)) {
        out += " ";
      }
      out += UNICODE_FRACTIONS[ch];
    } else {
      out += ch;
    }
  }
  return out;
}

function parseAmount(raw: string): number | null {
  const s = normalizeFractions(raw).trim();
  if (s.length === 0) return null;

  // Mixed number: "1 1/2"
  const mixed = s.match(/^(\d+)\s+(\d+)\/(\d+)$/);
  if (mixed) {
    const whole = Number(mixed[1]);
    const num = Number(mixed[2]);
    const denom = Number(mixed[3]);
    if (denom === 0) return null;
    return whole + num / denom;
  }

  // Vulgar fraction: "1/2"
  const frac = s.match(/^(\d+)\/(\d+)$/);
  if (frac) {
    const num = Number(frac[1]);
    const denom = Number(frac[2]);
    if (denom === 0) return null;
    return num / denom;
  }

  // Integer or decimal: "2", "2.5", ".5"
  if (/^\d+(\.\d+)?$/.test(s) || /^\.\d+$/.test(s)) {
    return Number(s);
  }

  return null;
}

function formatScaled(value: number): string {
  // Integer (with float tolerance)
  const rounded = Math.round(value);
  if (Math.abs(value - rounded) < FLOAT_TOL) {
    return String(rounded);
  }

  // Unit fraction 1/N for N in {2,3,4,6,8}
  for (const denom of UNIT_FRACTION_DENOMS) {
    if (Math.abs(value - 1 / denom) < FLOAT_TOL) {
      return `1/${denom}`;
    }
  }

  // Decimal, up to 2 places, trailing zeros stripped
  let out = value.toFixed(2);
  out = out.replace(/\.?0+$/, "");
  return out;
}

/**
 * Multiply an ingredient amount string by `factor` and re-format.
 *
 * Returns the original string unchanged when:
 *   - the amount cannot be parsed (free text, ranges, empty),
 *   - `factor` is not a positive finite number,
 *   - `factor === 1`.
 */
export function scaleAmount(
  amount: string | null | undefined,
  factor: number,
): string {
  if (amount == null || amount === "") return amount ?? "";
  if (!Number.isFinite(factor) || factor <= 0) return amount;
  if (factor === 1) return amount;

  const parsed = parseAmount(amount);
  if (parsed === null) return amount;

  return formatScaled(parsed * factor);
}
```

- [ ] **Step 2: Verify it typechecks**

Run: `make lint`
Expected: no new errors. (There may be unrelated existing warnings; the module itself should be clean.)

- [ ] **Step 3: Commit**

```bash
git add ramekin-ui/src/utils/scaleAmount.ts
git commit -m "$(cat <<'EOF'
Add scaleAmount utility

Pure function that parses an ingredient amount string, multiplies by a
factor, and formats the result as an integer, unit fraction, or decimal
per the design rule. Free-text and range strings pass through unchanged.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Bootstrap Playwright test file with a recipe fixture

This adds the test scaffolding (a fixture that creates a user + a recipe with one ingredient per representative shape) and a smoke test that the recipe page loads with the original amounts. Subsequent tasks add tests on top.

**Files:**
- Create: `tests/ui/test_recipe_scale.py`

- [ ] **Step 1: Write the fixture and smoke test**

Create `tests/ui/test_recipe_scale.py` with this exact content:

```python
"""
Tests for the recipe-view scale control: scaling ingredient amounts and
the Serves: line, plus the integration with the Add to Shopping List flow.
"""

import os
import uuid
from typing import List

import pytest
from playwright.sync_api import Page, expect

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, RecipesApi, ShoppingListApi
from ramekin_client.models import (
    CreateRecipeRequest,
    Ingredient,
    Measurement,
    SignupRequest,
)


# Each ingredient covers one representative amount shape. Order matters because
# the test asserts on the rendered list by index.
SCALE_TEST_INGREDIENTS: List[Ingredient] = [
    Ingredient(item="flour", measurements=[Measurement(amount="2", unit="cups")]),
    Ingredient(item="sugar", measurements=[Measurement(amount="1/2", unit="cup")]),
    Ingredient(item="butter", measurements=[Measurement(amount="1 1/2", unit="sticks")]),
    Ingredient(item="milk", measurements=[Measurement(amount="2.5", unit="cups")]),
    Ingredient(item="eggs", measurements=[Measurement(amount="3", unit=None)]),
    Ingredient(item="salt", measurements=[Measurement(amount="to taste", unit=None)]),
    Ingredient(item="bay leaves", measurements=[Measurement(amount="6-8", unit=None)]),
]


def _sign_up(api_url: str) -> tuple[str, str, str]:
    """Sign up a fresh user and return (username, password, token)."""
    username = f"scale_{uuid.uuid4().hex[:8]}"
    password = "testpass123"
    config = Configuration(host=api_url)
    with ApiClient(config) as client:
        auth_api = AuthApi(client)
        signup = auth_api.signup(SignupRequest(username=username, password=password))
    return username, password, signup.token


def _authed_client(api_url: str, token: str) -> ApiClient:
    config = Configuration(host=api_url)
    config.access_token = token
    return ApiClient(config)


@pytest.fixture
def scale_recipe(api_url: str, ui_url: str, page: Page):
    """Create a user with a known scale-test recipe and log the page in.

    Yields (recipe_id, token) so tests can hit the API directly.
    """
    username, password, token = _sign_up(api_url)
    with _authed_client(api_url, token) as client:
        recipes_api = RecipesApi(client)
        recipe = recipes_api.create_recipe(
            CreateRecipeRequest(
                title="Scale Test Recipe",
                instructions="Mix and bake.",
                ingredients=SCALE_TEST_INGREDIENTS,
                servings="4",
            )
        )

    # Log in via UI
    page.goto(ui_url)
    page.wait_for_selector("input[type='text']")
    page.fill("input[type='text']", username)
    page.fill("input[type='password']", password)
    page.click("button[type='submit']")
    page.wait_for_selector(".recipe-card")

    # Navigate to the recipe view
    page.goto(f"{ui_url.rstrip('/')}/recipes/{recipe.id}")
    page.wait_for_selector(".ingredients-list")

    yield recipe.id, token


def _amount_texts(page: Page) -> List[str]:
    """Return the visible primary-amount text for each rendered ingredient row."""
    return page.locator(".ingredients-list li .amount").all_text_contents()


def test_recipe_loads_with_original_amounts(scale_recipe, page: Page):
    recipe_id, _token = scale_recipe
    amounts = _amount_texts(page)
    # `to taste` and `6-8` have no .amount span (or render unchanged); we only
    # check the parseable rows here.
    assert "2" in amounts
    assert "1/2" in amounts
    assert "1 1/2" in amounts
    assert "2.5" in amounts
    assert "3" in amounts
    expect(page.locator(".recipe-metadata")).to_contain_text("Serves:")
    expect(page.locator(".recipe-metadata")).to_contain_text("4")
```

- [ ] **Step 2: Run the test suite**

Run: `make test-ui`
Expected: `test_recipe_loads_with_original_amounts` PASSES (it asserts existing behavior). Other tests in the suite should also still pass.

- [ ] **Step 3: Commit**

```bash
git add tests/ui/test_recipe_scale.py
git commit -m "$(cat <<'EOF'
Add Playwright fixture for the recipe-scale feature

Creates a fresh user with a recipe whose ingredients exercise every
amount shape the scaler will encounter (integer, fraction, mixed,
decimal, free-text, range), plus a smoke test that the recipe page
renders the original amounts.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Add `<ScaleControls>` strip and URL plumbing (no scaling yet)

Render the control strip and wire up `?scale=N` URL state. At the end of this task the URL changes when the user clicks a preset, but ingredient amounts still render unchanged — that's wired in Task 4.

**Files:**
- Modify: `ramekin-ui/src/pages/ViewRecipePage.tsx`
- Modify: `ramekin-ui/src/App.css`
- Modify: `tests/ui/test_recipe_scale.py`

- [ ] **Step 1: Write the failing UI test**

Append to `tests/ui/test_recipe_scale.py`:

```python
def test_clicking_preset_updates_url_and_active_state(scale_recipe, page: Page):
    recipe_id, _token = scale_recipe
    # Default state: no ?scale= param, 1× preset is active.
    assert "scale=" not in page.url
    one_x = page.locator(".scale-preset", has_text="1×")
    expect(one_x).to_have_class("scale-preset active")

    # Click 2× → URL gets ?scale=2, 2× preset active, 1× no longer.
    page.locator(".scale-preset", has_text="2×").click()
    page.wait_for_url("**/?scale=2")
    expect(page.locator(".scale-preset", has_text="2×")).to_have_class(
        "scale-preset active"
    )
    expect(one_x).not_to_have_class("scale-preset active")

    # Click 1× → URL clears the param, 1× active again.
    one_x.click()
    page.wait_for_url(lambda url: "scale=" not in url)
    expect(one_x).to_have_class("scale-preset active")


def test_custom_input_overrides_presets(scale_recipe, page: Page):
    recipe_id, _token = scale_recipe
    custom = page.locator(".scale-custom-input")
    # 1.5 doesn't match any preset.
    custom.fill("1.5")
    custom.press("Enter")
    page.wait_for_url("**/?scale=1.5")
    for preset_label in ["¼×", "½×", "1×", "2×", "3×"]:
        expect(
            page.locator(".scale-preset", has_text=preset_label)
        ).not_to_have_class("scale-preset active")
```

- [ ] **Step 2: Run to confirm it fails**

Run: `make test-ui`
Expected: the two new tests FAIL because `.scale-preset` and `.scale-custom-input` selectors don't exist yet.

- [ ] **Step 3: Add the scale signal and `<ScaleControls>` rendering**

In `ramekin-ui/src/pages/ViewRecipePage.tsx`:

(a) Just below the existing `const [searchParams, setSearchParams] = useSearchParams();` line near the top of `ViewRecipePage`, add:

```tsx
  const SCALE_PRESETS = [
    { value: 0.25, label: "¼×" },
    { value: 0.5, label: "½×" },
    { value: 1, label: "1×" },
    { value: 2, label: "2×" },
    { value: 3, label: "3×" },
  ];

  const scale = () => {
    const raw = searchParams.scale;
    const v = typeof raw === "string" ? Number(raw) : NaN;
    return Number.isFinite(v) && v > 0 ? v : 1;
  };

  const setScale = (v: number) => {
    if (!Number.isFinite(v) || v <= 0) return;
    setSearchParams({ scale: v === 1 ? undefined : String(v) });
  };

  const [customScaleInput, setCustomScaleInput] = createSignal("");
  const applyCustomScale = () => {
    const v = Number(customScaleInput());
    if (Number.isFinite(v) && v > 0) {
      setScale(v);
    }
  };
```

(b) Inside the `<section class="recipe-section">` that contains `<h3>Ingredients</h3>` (around line 876–877 in the current file), insert the controls strip immediately after the `<h3>` element and before the `<For each={groupIngredientsBySection(...)}>`:

```tsx
                    <div class="scale-controls">
                      <span class="scale-label">Scale:</span>
                      <For each={SCALE_PRESETS}>
                        {(preset) => (
                          <button
                            type="button"
                            class={
                              scale() === preset.value
                                ? "scale-preset active"
                                : "scale-preset"
                            }
                            onClick={() => {
                              setCustomScaleInput("");
                              setScale(preset.value);
                            }}
                          >
                            {preset.label}
                          </button>
                        )}
                      </For>
                      <input
                        type="number"
                        step="0.25"
                        min="0"
                        class="scale-custom-input"
                        placeholder="Custom"
                        value={customScaleInput()}
                        onInput={(e) =>
                          setCustomScaleInput(e.currentTarget.value)
                        }
                        onKeyDown={(e) => {
                          if (e.key === "Enter") applyCustomScale();
                        }}
                        onBlur={applyCustomScale}
                      />
                    </div>
```

- [ ] **Step 4: Add the CSS**

Append to `ramekin-ui/src/App.css`:

```css
.scale-controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.375rem;
  margin: 0.5rem 0 0.75rem;
  font-size: 0.875rem;
}

.scale-controls .scale-label {
  color: var(--text-secondary);
  margin-right: 0.25rem;
}

.scale-preset {
  background: var(--surface-2);
  border: 1px solid var(--border-subtle);
  border-radius: 4px;
  padding: 0.2rem 0.55rem;
  font-size: 0.85rem;
  cursor: pointer;
  color: var(--text-primary);
}

.scale-preset:hover {
  background: var(--surface-3, var(--surface-2));
}

.scale-preset.active {
  background: var(--accent, #3a6ea5);
  color: #fff;
  border-color: var(--accent, #3a6ea5);
}

.scale-custom-input {
  width: 4.5rem;
  padding: 0.2rem 0.4rem;
  font-size: 0.85rem;
  border: 1px solid var(--border-subtle);
  border-radius: 4px;
  background: var(--surface-1, var(--surface-2));
  color: var(--text-primary);
  margin-left: 0.25rem;
}
```

(If `--surface-3` or `--accent` don't exist, the fallbacks in the `var()` calls cover the styling.)

- [ ] **Step 5: Run lint and tests**

Run: `make lint`
Expected: passes.

Run: `make test-ui`
Expected: both new tests PASS. The original `test_recipe_loads_with_original_amounts` still PASSES because amounts haven't changed yet.

- [ ] **Step 6: Commit**

```bash
git add ramekin-ui/src/pages/ViewRecipePage.tsx ramekin-ui/src/App.css tests/ui/test_recipe_scale.py
git commit -m "$(cat <<'EOF'
Add scale-controls strip and URL state to recipe view

Renders preset buttons (¼×, ½×, 1×, 2×, 3×) and a custom number input
above the ingredients list. The active scale lives in the ?scale= URL
search param so refresh and link-sharing preserve it. This change is UI
plumbing only — ingredient amounts still render unscaled until the
next task.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Apply the scale to ingredient amounts and the `Serves:` line

Pipe `scale()` through every rendered amount and through the `Serves:` value, and add the `(scaled N×)` badge next to the Ingredients heading and the Serves: line.

**Files:**
- Modify: `ramekin-ui/src/pages/ViewRecipePage.tsx`
- Modify: `ramekin-ui/src/App.css`
- Modify: `tests/ui/test_recipe_scale.py`

- [ ] **Step 1: Write the failing UI tests**

Append to `tests/ui/test_recipe_scale.py`:

```python
def test_scale_2x_doubles_amounts(scale_recipe, page: Page):
    recipe_id, _token = scale_recipe
    page.locator(".scale-preset", has_text="2×").click()
    page.wait_for_url("**/?scale=2")

    amounts = _amount_texts(page)
    # 2 → 4, 1/2 → 1, 1 1/2 → 3, 2.5 → 5, 3 → 6
    assert "4" in amounts
    assert "1" in amounts
    assert "3" in amounts
    assert "5" in amounts
    assert "6" in amounts
    # Unparseable amounts pass through unchanged.
    page_text = page.locator(".ingredients-list").inner_text()
    assert "to taste" in page_text
    assert "6-8" in page_text
    # Serves: 4 → 8
    expect(page.locator(".recipe-metadata")).to_contain_text("8")
    # Badge is shown (one near Ingredients heading, one near Serves: line).
    assert page.locator(".scale-badge").count() == 2
    expect(page.locator(".scale-badge").first).to_contain_text("scaled 2×")


def test_scale_half_halves_amounts(scale_recipe, page: Page):
    recipe_id, _token = scale_recipe
    page.locator(".scale-preset", has_text="½×").click()
    page.wait_for_url("**/?scale=0.5")

    amounts = _amount_texts(page)
    # 2 → 1, 1/2 → 1/4, 1 1/2 → 0.75, 2.5 → 1.25, 3 → 1.5
    assert "1" in amounts
    assert "1/4" in amounts
    assert "0.75" in amounts
    assert "1.25" in amounts
    assert "1.5" in amounts


def test_clicking_1x_clears_badge(scale_recipe, page: Page):
    recipe_id, _token = scale_recipe
    page.locator(".scale-preset", has_text="2×").click()
    page.wait_for_url("**/?scale=2")
    assert page.locator(".scale-badge").count() == 2

    page.locator(".scale-preset", has_text="1×").click()
    page.wait_for_url(lambda url: "scale=" not in url)
    expect(page.locator(".scale-badge")).to_have_count(0)
```

- [ ] **Step 2: Run to confirm they fail**

Run: `make test-ui`
Expected: the three new tests FAIL — amounts still render unscaled and the `.scale-badge` element doesn't exist yet.

- [ ] **Step 3: Wire `scaleAmount` into rendering**

In `ramekin-ui/src/pages/ViewRecipePage.tsx`:

(a) Add an import at the top, alongside the existing imports from utils:

```tsx
import { scaleAmount } from "../utils/scaleAmount";
```

(b) In the ingredients `<li>` (the `<For each={group.ingredients}>` body, around lines 889–921 of the current file), wrap every `amount` reference with `scaleAmount(...)`:

Replace this:

```tsx
                                  <Show when={ing.measurements[0]?.amount}>
                                    <span class="amount">
                                      {ing.measurements[0]?.amount}
                                    </span>{" "}
                                  </Show>
                                  <Show when={ing.measurements[0]?.unit}>
                                    <span class="unit">
                                      {ing.measurements[0]?.unit}
                                    </span>{" "}
                                  </Show>
                                  <Show when={ing.measurements.length > 1}>
                                    <span class="alt-measurement">
                                      (
                                      {ing.measurements
                                        .slice(1)
                                        .map((m) =>
                                          [m.amount, m.unit]
                                            .filter(Boolean)
                                            .join(" "),
                                        )
                                        .join(", ")}
                                      ){" "}
                                    </span>
                                  </Show>
```

with:

```tsx
                                  <Show when={ing.measurements[0]?.amount}>
                                    <span class="amount">
                                      {scaleAmount(
                                        ing.measurements[0]?.amount,
                                        scale(),
                                      )}
                                    </span>{" "}
                                  </Show>
                                  <Show when={ing.measurements[0]?.unit}>
                                    <span class="unit">
                                      {ing.measurements[0]?.unit}
                                    </span>{" "}
                                  </Show>
                                  <Show when={ing.measurements.length > 1}>
                                    <span class="alt-measurement">
                                      (
                                      {ing.measurements
                                        .slice(1)
                                        .map((m) =>
                                          [
                                            scaleAmount(m.amount, scale()),
                                            m.unit,
                                          ]
                                            .filter(Boolean)
                                            .join(" "),
                                        )
                                        .join(", ")}
                                      ){" "}
                                    </span>
                                  </Show>
```

(c) Replace the `Serves:` value rendering. The current block (around lines 835–840) is:

```tsx
                  <Show when={r().servings}>
                    <div class="recipe-metadata-item">
                      <span class="label">Serves:</span>
                      <span class="value">{r().servings}</span>
                    </div>
                  </Show>
```

Replace with:

```tsx
                  <Show when={r().servings}>
                    <div class="recipe-metadata-item">
                      <span class="label">Serves:</span>
                      <span class="value">
                        {scaleAmount(r().servings, scale())}
                      </span>
                      <Show when={scale() !== 1}>
                        <span class="scale-badge">
                          scaled {formatScaleLabel(scale())}
                        </span>
                      </Show>
                    </div>
                  </Show>
```

(d) Add the `scale-badge` to the Ingredients heading (around line 877):

Replace:

```tsx
                  <section class="recipe-section">
                    <h3>Ingredients</h3>
```

with:

```tsx
                  <section class="recipe-section">
                    <h3>
                      Ingredients
                      <Show when={scale() !== 1}>
                        {" "}
                        <span class="scale-badge">
                          scaled {formatScaleLabel(scale())}
                        </span>
                      </Show>
                    </h3>
```

(e) Add the `formatScaleLabel` helper near the top of the `ViewRecipePage` function body, alongside `scale()` / `setScale()`:

```tsx
  const formatScaleLabel = (v: number): string => {
    // Render the multiplier as ½×, ⅓×, ¼× when it matches a unit fraction; otherwise as a decimal.
    const pretty: Record<string, string> = {
      "0.25": "¼",
      "0.5": "½",
      "0.3333333333333333": "⅓",
      "0.6666666666666666": "⅔",
    };
    const key = String(v);
    if (key in pretty) return `${pretty[key]}×`;
    // Drop trailing zeros and the trailing dot.
    const trimmed = v.toFixed(2).replace(/\.?0+$/, "");
    return `${trimmed}×`;
  };
```

- [ ] **Step 4: Add `.scale-badge` styles**

Append to `ramekin-ui/src/App.css`:

```css
.scale-badge {
  display: inline-block;
  margin-left: 0.5rem;
  padding: 0.05rem 0.4rem;
  font-size: 0.75rem;
  font-weight: 500;
  color: var(--text-secondary);
  background: var(--surface-2);
  border: 1px solid var(--border-subtle);
  border-radius: 999px;
  vertical-align: middle;
}
```

- [ ] **Step 5: Run lint and tests**

Run: `make lint`
Expected: passes.

Run: `make test-ui`
Expected: all six tests in `test_recipe_scale.py` PASS.

- [ ] **Step 6: Commit**

```bash
git add ramekin-ui/src/pages/ViewRecipePage.tsx ramekin-ui/src/App.css tests/ui/test_recipe_scale.py
git commit -m "$(cat <<'EOF'
Apply scale factor to rendered ingredient amounts and Serves: value

Pipes every measurement amount and the Serves: value through scaleAmount
using the active scale signal. Adds a (scaled N×) badge next to the
Ingredients heading and the Serves: line so the reader can tell at a
glance that they're not looking at canonical amounts. Free-text and
range strings continue to pass through unchanged.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Carry the scale through "Add to Shopping List"

Pass the active scale to `AddToShoppingListModal`, scale amounts in the in-modal preview text and in the API payload.

**Files:**
- Modify: `ramekin-ui/src/components/AddToShoppingListModal.tsx`
- Modify: `ramekin-ui/src/pages/ViewRecipePage.tsx`
- Modify: `tests/ui/test_recipe_scale.py`

- [ ] **Step 1: Write the failing test**

Append to `tests/ui/test_recipe_scale.py`:

```python
def test_shopping_list_uses_scaled_amounts(scale_recipe, api_url, page: Page):
    recipe_id, token = scale_recipe

    # Scale to 2× then open the shopping-list modal.
    page.locator(".scale-preset", has_text="2×").click()
    page.wait_for_url("**/?scale=2")
    page.locator("button", has_text="Add to Shopping List").click()
    page.wait_for_selector(".add-shopping-modal")

    # Confirm the modal shows scaled amounts.
    modal_text = page.locator(".add-shopping-modal").inner_text()
    assert "4 cups flour" in modal_text  # 2 → 4
    assert "1 cup sugar" in modal_text  # 1/2 → 1

    # Submit.
    page.locator(".add-shopping-modal button", has_text="Add").click()
    page.wait_for_selector(".add-shopping-success")

    # Read items back via the API.
    config = Configuration(host=api_url)
    config.access_token = token
    with ApiClient(config) as client:
        items = ShoppingListApi(client).list_items().items

    by_item = {it.item: it.amount for it in items}
    assert by_item["flour"] == "4 cups"
    assert by_item["sugar"] == "1 cup"
    assert by_item["butter"] == "3 sticks"
    assert by_item["milk"] == "5 cups"
    assert by_item["eggs"] == "6"
    # Free-text and range pass through.
    assert by_item["salt"] == "to taste"
    assert by_item["bay leaves"] == "6-8"
```

> **Note:** `list_items()` is the generated Python client method on `ShoppingListApi` — verified against `tests/test_shopping_list.py`. The response has `.items` (list of `ShoppingListItemResponse`).

- [ ] **Step 2: Run to confirm it fails**

Run: `make test-ui`
Expected: `test_shopping_list_uses_scaled_amounts` FAILS — the modal currently displays and posts unscaled amounts.

- [ ] **Step 3: Update `AddToShoppingListModal` to accept a scale prop**

In `ramekin-ui/src/components/AddToShoppingListModal.tsx`:

(a) Add a `scaleAmount` import at the top alongside the existing imports:

```tsx
import { scaleAmount } from "../utils/scaleAmount";
```

(b) Extend the props interface (line 7–11):

```tsx
interface AddToShoppingListModalProps {
  isOpen: () => boolean;
  onClose: () => void;
  recipe: RecipeResponse;
  scale?: () => number;
}
```

(c) Replace the two formatter helpers at the top of the file (lines 13–37) with versions that take a scale:

```tsx
function formatIngredient(ing: Ingredient, scale: number): string {
  const parts: string[] = [];
  const amount = scaleAmount(ing.measurements[0]?.amount, scale);
  if (amount) {
    parts.push(amount);
  }
  if (ing.measurements[0]?.unit) {
    parts.push(ing.measurements[0].unit);
  }
  parts.push(ing.item);
  if (ing.note) {
    parts.push(`(${ing.note})`);
  }
  return parts.join(" ");
}

function formatAmount(ing: Ingredient, scale: number): string | undefined {
  const parts: string[] = [];
  const amount = scaleAmount(ing.measurements[0]?.amount, scale);
  if (amount) {
    parts.push(amount);
  }
  if (ing.measurements[0]?.unit) {
    parts.push(ing.measurements[0].unit);
  }
  return parts.length > 0 ? parts.join(" ") : undefined;
}
```

(d) Add a local accessor near the top of the component body, replacing the existing `const ingredients = () => props.recipe.ingredients ?? [];` line (line 51) with:

```tsx
  const scale = () => props.scale?.() ?? 1;
  const ingredients = () => props.recipe.ingredients ?? [];
```

(e) Update the two call sites of `formatIngredient` / `formatAmount`:

In `handleAdd` (around lines 92–99), change:

```tsx
        .map((ing) => ({
          item: ing.item,
          amount: formatAmount(ing),
          sourceRecipeId: props.recipe.id,
          sourceRecipeTitle: props.recipe.title,
        }));
```

to:

```tsx
        .map((ing) => ({
          item: ing.item,
          amount: formatAmount(ing, scale()),
          sourceRecipeId: props.recipe.id,
          sourceRecipeTitle: props.recipe.title,
        }));
```

In the `<For each={ingredients()}>` body (around line 175), change:

```tsx
                  <span class="ingredient-text">{formatIngredient(ing)}</span>
```

to:

```tsx
                  <span class="ingredient-text">
                    {formatIngredient(ing, scale())}
                  </span>
```

- [ ] **Step 4: Pass scale from `ViewRecipePage`**

In `ramekin-ui/src/pages/ViewRecipePage.tsx`, find the `<AddToShoppingListModal ... />` JSX element (around line 1027) and add the `scale` prop:

```tsx
            <AddToShoppingListModal
              isOpen={showShoppingListModal}
              onClose={() => setShowShoppingListModal(false)}
              recipe={r()}
              scale={scale}
            />
```

- [ ] **Step 5: Run lint and tests**

Run: `make lint`
Expected: passes.

Run: `make test-ui`
Expected: all seven tests in `test_recipe_scale.py` PASS.

- [ ] **Step 6: Commit**

```bash
git add ramekin-ui/src/components/AddToShoppingListModal.tsx ramekin-ui/src/pages/ViewRecipePage.tsx tests/ui/test_recipe_scale.py
git commit -m "$(cat <<'EOF'
Carry recipe scale through Add to Shopping List

The shopping-list modal now scales amounts in both its in-modal preview
and its API payload, using the active scale signal supplied by the
recipe view. A 2× recipe added to the shopping list lands with doubled
amounts in the database.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Final lint pass and stack-level smoke

A safety net: confirm no regressions across the rest of the UI suite, then commit any cleanup.

**Files:**
- (Possibly) any file touched in earlier tasks if lint surfaces fixes.

- [ ] **Step 1: Run `make lint`**

Run: `make lint`
Expected: passes with no warnings introduced by this branch. Fix anything related; do NOT bypass with `// eslint-disable` or noqa.

- [ ] **Step 2: Run the full UI suite**

Run: `make test-ui`
Expected: all UI tests pass, including the seven new tests in `test_recipe_scale.py` and pre-existing tests in `test_smoke.py` and `test_scrape_status_page.py`.

- [ ] **Step 3: Manual smoke (5 min)**

Start the dev stack with `make dev` (or `make dev-headless`), log in as the seed user (`t` / `t`), open any recipe, and try:
- Click `2×` and `½×`; confirm amounts scale and the badge appears.
- Refresh the page on `?scale=2`; confirm the scale persists.
- Type `1.25` in the custom input and press Enter; confirm amounts scale.
- Add a scaled recipe to the shopping list; navigate to the shopping list page (or query `/api/shopping-list`); confirm amounts are scaled.
- Type `0` or `-1` in the custom input; confirm it's ignored and the prior scale stays.

- [ ] **Step 4: Commit any cleanup**

If lint or smoke surfaced fixes, commit them:

```bash
git add -A
git commit -m "$(cat <<'EOF'
Polish recipe-scale feature

Lint/smoke fixes from final pass.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

If nothing changed, skip the commit.

---

## Spec coverage check

| Spec section / requirement | Implemented in |
|---|---|
| Multiplier-based scaling (whole numbers, 1/N, decimals) | Task 1 (`scaleAmount`) |
| Preset buttons `¼×`, `½×`, `1×`, `2×`, `3×` | Task 3 |
| Custom decimal input | Task 3 |
| URL state via `?scale=N` | Task 3 |
| Reject invalid scales (≤0, NaN) | Task 1 + Task 3 (validation in setter) |
| Scale primary measurement amounts | Task 4 |
| Scale alternative measurements | Task 4 |
| Scale `Serves:` when parseable | Task 4 |
| `(scaled N×)` badge near Ingredients heading and Serves: | Task 4 |
| Free text / ranges pass through | Task 1 (parser returns `null`) + tested in Task 4 |
| Shopping-list integration uses scaled amounts | Task 5 |
| Playwright coverage for parser shapes via fixture | Task 2 (fixture) + Task 4/5 (assertions) |
| Lint + suite green | Task 6 |
| No backend / API / generated-client changes | Verified by Task 6 (no Rust diff) |
