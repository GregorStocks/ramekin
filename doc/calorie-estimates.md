# Recipe calorie estimates

Estimates are calculated on the server from the displayed ingredient snapshot.
They are separate from imported `nutritional_info`, which remains editable text.
The authenticated `POST /api/recipes/estimate-calories` endpoint takes ingredients,
the original servings string, and a positive scale (at most 1,000,000). It does
not save data or use an LLM. Both clients render the returned summaries and
unknown reasons, and request a new estimate when the displayed recipe or scale
changes. Historical versions therefore use their own ingredients.

## Data and regeneration

Source: USDA FoodData Central, SR Legacy April 2018, public domain (CC0).
See https://fdc.nal.usda.gov/download-datasets/ and
https://fdc.nal.usda.gov/api-guide/. This is the final SR Legacy release.

Run `make nutrition-import`. The importer downloads the fixed USDA CSV archive
into `.cache/`, then generates `ramekin-core/src/nutrition/data.json`. The output
records the source URL, archive SHA-256, FDC food IDs, food descriptions, and
nutrient 1008 (Energy) in kcal per 100 grams of edible food. It is committed so
runtime estimation needs no network. Re-importing a changed archive fails until
the source change is explicitly reviewed. No generated data is hand-edited.

Volume conversion uses only a portion explicitly labeled `cup` on the **same FDC
food record**, normalized by the portion's amount. Portions with zero amounts
are unusable and excluded. A food has a cup weight only if all usable plain-cup
records agree. Modified cups, such as `cup, chopped`, are not silently substituted.
Missing portions produce an explicit unknown. This intentionally avoids the
density database's broader ingredient rewrites and guesses about preparation.

## Matching and quantities

- Normalize case and whitespace only. Match a reviewed alias in `aliases.json`
  or a unique exact USDA description. There is no fuzzy matching or modifier
  stripping. Multiple exact records and explicitly null aliases are ambiguous.
- Aliases pin specific FDC IDs. For example, all-purpose flour selects unbleached
  enriched all-purpose flour, whole milk selects 3.25% milkfat with vitamin D,
  and eggs select raw whole fresh egg. Raw onion and garlic names use raw food
  records. These are documented generic reference foods, not brand-specific data.
  Broad names such as yogurt, flour, rice, oil, and milk remain ambiguous.
- Accept nonnegative decimals, fractions, mixed numbers, common Unicode
  fractions, and ordered ranges with hyphen, en dash, em dash, `to`, or `or`.
  Unsupported strings, negative values, reversed ranges and nonfinite values
  remain unknown. Each quantity is bounded at 10^12 for arithmetic safety.
- Mass units: g, kg, mg, oz (28.349523125 g), lb (453.59237 g), including their
  listed singular/plural spellings. Volume uses the existing US cup conversion
  constants for cup, tbsp, tsp, fluid ounce, pint, quart, gallon, ml and l.
  Counts and packages require a supported alternate measurement.
- Use the first supported measurement once; subsequent measurements are
  alternatives, not additional ingredients. This preserves precise primary
  amounts ahead of rounded gram alternatives. As with ingredient display,
  quantities are interpreted as the listed edible ingredient amounts; estimates
  do not model cooking losses, absorption, leftovers, or optional consumption.
- Sum lower and upper bounds separately and apply scale to both. Reject totals
  above 10^15 kcal. A real known zero (e.g. table salt) is distinct from no known
  ingredients, which returns null rather than zero.

The response version hashes the data and aliases together with the calculation
rule version. Bump the rule version when changing calculation behavior, including
changes to reused volume constants. Identical inputs and version give identical
results. Numeric fields retain calculation precision; summaries round exact
estimates to whole calories and range bounds outward.

## Presentation

Complete estimates say “Whole recipe: approximately … calories.” Partial results
say “Known ingredients: … calories, plus unknown calories from …” and explicitly
identify the subtotal as partial. Entirely unknown recipes have no numeric total.
Every unsupported ingredient includes its input index, name, and reason.

Only a positive numeric serving count, optionally prefixed by `serves` or suffixed
by `serving`/`servings`, is used for per-serving estimates. Yield text (`1 loaf`,
`4–6`) is deliberately not interpreted. Scaling
multiplies both ingredients and servings, so per-serving calories stay the same.
Partial per-serving estimates are labeled partial as well. Imported nutrition
text is displayed separately and never feeds the calculation.
