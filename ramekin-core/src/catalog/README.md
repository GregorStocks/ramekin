# Ingredient catalog

One place that turns a written ingredient name ("softened butter", "Dry  White
Wine") into a known food. Density lookup (`volume_to_weight.rs`) and calorie
estimates (`nutrition/`) both go through it, so a name the catalog learns helps
both. The shopping-list categorizer (`ingredient_categorizer.rs`) uses an
entry's catalog `category` when it has one. Until catalog entries carry
categories (`issues/*ingredient-catalog-step2*`), its keyword rules in
`data/ingredients.json` remain the fallback.

Coverage of every matcher is tracked in `data/ingredient-catalog-audit.md`
(`make ingredient-catalog-audit`); a change here should show up there.

## Resolution vs. attributes

`resolve(name)` answers *which food is this*. Each entry then carries its own
attributes, and either can be missing on purpose:

- `fdc_id`: the USDA food supplying calories.
- `grams_per_cup`: density for volume-to-weight conversion.

So "black pepper" resolves (its calories are known) but has no density, because
grind size changes the weight of a cup too much. Names that could mean several
foods ("cheese", "rice", "chicken", "yogurt") are ambiguous for every attribute.

A resolution is one of:

- `Entry`: one food or product.
  - **Products** (`kind: "product"`) are bought but not eaten: parchment paper,
    skewers. They carry only a shopping `category`, and calorie estimates skip them.
- `Compound`: several foods joined by "and", "&" or "and/or" on one line with one
  amount ("salt and pepper").
  - This is only tried once the whole name failed, so "half and half" stays one
    food, and every part must resolve to a specific food.
  - Density is unknown, since the mixture ratio is unknown.
  - The shopping category is the first food's.
- `NotFood`: a leftover section header (any name ending in ":") or a curated
  `not_food` phrase ("to serve"). Calorie estimates skip these lines.
- `Ambiguous` / `Unresolved`.

Each entry also has two negligibility attributes, derived from the data:

- **`zero_calorie`:** the linked food has 0 kcal/100 g (salt, water, baking soda).
  Any line of it is negligible.
- **`trace_ok`:** the linked food is a USDA "spices, …" food (dried spices, black
  pepper), or the curated entry sets `trace_ok: true`. A line of it is negligible
  only when it has no numeric amount ("to taste", no measurement) or a trace unit
  (pinch, dash). A real amount, like a cup of cumin, counts in full.

A compound line is negligible only if every one of its foods is negligible on
that line. Otherwise its calories are unknown.

## Lookup order

Names are lowercased and whitespace is collapsed. Then:

1. Exact match against the index, which holds, in priority order:
   - curated entry ids;
   - USDA stripped names;
   - unique USDA descriptions (a description shared by several foods is ambiguous);
   - curated aliases, which may not shadow any of the above and may only point
     at a non-alias name.
2. A singular/plural variant: add or remove "s", then "es", then "ies" ↔ "y".
3. Steps 1–2 again after stripping temperature and preparation modifiers
   ("softened ", ", sifted", …).
4. The name with trailing ", …" or "; …" clauses dropped one at a time
   ("kosher salt, presumably diamond" → "kosher salt"). A clause listing
   alternatives ("milk, dairy or non-dairy") resolves to the first-listed
   food, the same way the first measurement wins over later alternatives.
5. The name with leading size and preparation words (`LEADING_MODIFIERS`:
   "chopped", "fresh", "large", "boneless", …) dropped one word at a time, so
   "grated fresh ginger" tries "fresh ginger" before "ginger". Words that change
   the food ("ground", "dried", "light", "crushed", "whole") are never dropped.
   A *bare* name reached this way may not be a dried or ground spice unless the
   original said "dried" or "ground". "fresh rosemary" stays unknown rather than
   becoming dried rosemary. A curated alias that keeps a prep word ("grated
   nutmeg") is an explicit choice and still counts.

Only a match to a specific food ends the search. Trimming never settles for
a name that isn't a food ("boneless, skinless …" is never cut to "boneless"),
and an ambiguous hit still lets a later step find a specific food. A trailing
" to taste" is stripped like the other modifiers. If no single food matches,
the name is tried as a compound (see above).

## Data files

### `data/usda.json` (generated; do not edit)

`make catalog-import` runs `scripts/import-catalog.py`. It downloads the fixed
USDA FoodData Central SR Legacy April 2018 CSV archive (public domain, CC0),
verifies its SHA-256 (cached at `.cache/nutrition-sr-legacy-2018-04.zip`; a
mismatch fails until the source change is reviewed), and writes:

- `foods`: one record per food with `fdc_id`, `description` (lowercased,
  whitespace collapsed), `kcal_per_100g` (nutrient 1008, Energy), and
  `grams_per_cup`.
- `names`: stripped name → `fdc_id`.

Density rules:

- Divide portion gram weight by amount. Prefer cup portions, then tablespoons
  (16 per cup), then teaspoons (48 per cup), and average within the preferred unit.
- Accept `cup`, `cups`, `cup, ...` and `cup (...)`. Skip chip portions and other units.
- Seven zero-amount cup rows in this release are pinned in `EXCLUDED_PORTIONS`.
  Any other malformed supported-volume portion fails the import.

Name rules:

- Lowercase, collapse whitespace, and strip the trailing suffixes in `NAME_SUFFIXES`
  (", raw", ", dry", ", enriched…") in list order.
- When several foods share a stripped name, the first food with a density in
  portion-table order wins. Otherwise the first food in food-table order wins.

The importer rejects empty tables, duplicate IDs, unknown food references, and
invalid energies. Repeated imports produce identical bytes. Its offline tests
are `tests/test_catalog_import.py`.

### `data/curated.json` (hand-maintained)

```json
{
  "entries": {
    "diamond crystal kosher salt": {
      "fdc_id": 173468,
      "grams_per_cup": { "value": 137.0, "source": "Serious Eats", "url": "https://..." }
    },
    "sea salt": {
      "fdc_id": 173468,
      "grams_per_cup": { "none": "Crystal size varies too much ..." }
    },
    "parchment paper": { "kind": "product", "category": "Household" }
  },
  "aliases": {
    "kosher salt": "diamond crystal kosher salt",
    "cheese": null
  },
  "not_food": { "to serve": "A serving suggestion, not an ingredient." },
  "rewrites": { "salt": "kosher salt" }
}
```

- `entries` override USDA foods of the same name.
  - `fdc_id` links the food whose calories apply. Omit it when USDA has no
    equivalent (e.g. mirin), so calories stay unknown rather than wrong.
  - `grams_per_cup` is either a cited value, or `{"none": reason}` to suppress
    the linked food's density. Omit it to inherit the linked food's density.
  - Every value needs a `source`.
  - The 23 manual baking values still lack individual citations. See
    `issues/p3-cite-embedded-manual-density-values`.
  - `category` (one of `ingredient_categorizer::CATEGORIES`) overrides the
    keyword categorizer.
  - `trace_ok: true` marks a food commonly listed without an amount, for foods
    the "spices, …" rule doesn't cover.
  - `kind: "product"` entries need a `category` and may not have an `fdc_id` or
    `grams_per_cup`.
- `aliases` map a name to an entry id, USDA stripped name, or unique USDA
  description. A `null` alias marks a name as ambiguous. An alias decides food
  identity for *every* attribute. When USDA has the food itself, point at it
  (e.g. "beef broth"). If only its density is missing, add an entry linking the
  real food (`fdc_id`) with an approximate `grams_per_cup` and a source
  explaining it (see "dried thyme", "greek yogurt"). Aliasing to a stand-in
  food is only for foods USDA lacks entirely ("dijon mustard" → yellow mustard,
  "shaoxing wine" → sake), where the stand-in supplies every attribute.
- `not_food` lists phrases that are not ingredients at all, with the reason.
  Names ending in ":" are headers and need no entry.
- `rewrites` rename the stored ingredient at import ("salt" → "kosher salt").

The loader asserts that keys are normalized, targets exist, aliases don't
shadow names, and densities are finite and positive. `catalog::version()`
hashes both files and the rule version; bump `RULE_VERSION` in `mod.rs` when
resolution behavior changes.

## Classification passes

Aliases beyond the hand-curated core come from Claude Code subagent passes over
the names the catalog didn't resolve. The procedure, and how to rerun it, is in
`docs/agent/catalog-classification.md`.

| Date | Tier | Model / harness | Applied | Skipped |
| --- | --- | --- | --- | --- |
| 2026-09-28 | Top 2,000 unresolved names (seen ≥ 4 times across pipeline and Paprika fixtures and prod) | Claude Opus 5.5 via a Claude Code Workflow: 8 classifiers + 3 verifiers | 1,670 aliases, 16 ambiguous, 6 not-food, 4 products, 1 entry | 303 |

- **Verification:** the verifiers checked every mapping to a ≥ 300 kcal/100 g food (543), a
  ~1/7 sample of the rest, and consistency across all decisions. They corrected 34 decisions.
  On review, 8 generic dish names ("soup", "meatballs") were changed from not-food to skip,
  so a real "1 lb frozen meatballs" line is never silently dropped.
- **Pipeline-fixture effect:**
  - recognized names on food lines went from 54.0% to 77.8%;
  - calories computed from 44.7% to 58.3%;
  - volume lines with a density from 62.9% to 77.5%.
