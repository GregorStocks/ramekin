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

Ingredient names are matched by the shared ingredient catalog
(`ramekin-core/src/catalog/`, see its README), which also drives volume-to-weight
density. `make catalog-import` downloads the fixed USDA CSV archive into
`.cache/` and generates `ramekin-core/src/catalog/data/usda.json`. It records the
source URL, archive SHA-256, FDC food IDs, descriptions, nutrient 1008 (Energy)
in kcal per 100 grams of edible food, and grams per cup. It is committed so
runtime estimation needs no network. Re-importing a changed archive fails until
the source change is explicitly reviewed. No generated data is hand-edited.

Volume conversion uses the matched catalog entry's grams per cup. This is either
a cited curated value or the USDA food's own cup, tablespoon or teaspoon portions
(zero-amount rows excluded). An entry without a density produces an explicit
"Missing density for this food" unknown.

## Matching and quantities

- A name resolves through the catalog: case and whitespace normalization,
  curated aliases, USDA names and descriptions, singular/plural variants, and
  temperature/preparation modifier stripping. The matched entry's `fdc_id`
  supplies calories.
  - An entry with no linked USDA food, such as mirin, is "No supported
    nutrition match".
  - Explicitly ambiguous names are "Ambiguous ingredient": cheese, rice, chicken,
    yogurt, and USDA descriptions shared by several foods.
- Common broad names use pinned defaults shared with density: sugar → granulated
  sugar, flour → unbleached enriched all-purpose flour, butter → unsalted butter,
  oil → vegetable (soybean) oil, milk → whole 3.25% milk, salt → kosher salt
  (0 kcal). These are generic reference foods, not brand-specific data.
- Accept nonnegative decimals, fractions, mixed numbers, common Unicode
  fractions, and ordered ranges with hyphen, en dash, em dash, `to`, or `or`.
  Dot and comma decimal separators follow the client scaling contract: `1,5`,
  `,5`, and `0,125` are decimals; ambiguous grouping (`1,000`), multiple commas,
  and mixed dot/comma strings are unsupported.
  Hyphenated mixed numbers (`1-1/2`) mean 1.5; ranges may also contain mixed
  endpoints (`1-1/2-2`). Sum every term of compound measurements such as
  `1 tablespoon plus 1 teaspoon`, including chains and ranges. Each term must
  have a supported quantity and unit; unsupported terms make the ingredient
  unknown rather than producing an incomplete ingredient contribution.
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

The response version combines the calculation rule version with the catalog
version (a hash of the catalog data and its resolution rule version). Bump the
calculation rule version when changing calculation behavior, including changes
to reused volume constants. Identical inputs and version give identical
results. Numeric fields retain calculation precision; summaries round exact
estimates to whole calories and range bounds outward.

## Negligible, compound, and non-food lines

Some lines are known without a usable amount (rules in the catalog README):

- **Zero-calorie foods** (salt, water, baking soda) contribute 0 kcal on any line.
- **Spices** (USDA "spices, …", including black pepper) contribute 0 kcal when
  the line has no numeric amount ("to taste") or only a pinch or dash. A real
  amount is computed normally.
- **Compound lines** ("salt and pepper", one amount for several foods) contribute
  0 kcal only if every food is negligible on that line. Otherwise they are
  unknown with "Several ingredients share one amount", because one amount can't
  be split between foods.
- **Leftover section headers** (names ending in ":"), serving notes ("to serve")
  and products such as parchment paper are skipped. They add nothing and are not
  listed as unknown. A recipe of only such lines is "No ingredients to estimate."

## Presentation

Complete estimates say “Whole recipe: approximately … calories.” Partial results
say “Known ingredients: … calories, plus unknown calories from …” and explicitly
identify the subtotal as partial. Entirely unknown recipes have no numeric total.
Every unsupported ingredient includes its input index, name, and reason.

Only a positive numeric serving count, optionally prefixed by `serves`, `Serves:`,
or `Servings:` (case-insensitive), or suffixed by `serving`/`servings`, is used for
per-serving estimates. Yield text (`1 loaf`,
`4–6`) is deliberately not interpreted. Scaling
multiplies both ingredients and servings, so per-serving calories stay the same.
Partial per-serving estimates are labeled partial as well. Imported nutrition
text is displayed separately and never feeds the calculation.

Web and iOS also scale displayed ranges, mixed numbers, compound measurements,
and explicit serving-count labels. This presentation logic remains duplicated
in their existing scale helpers and is pinned by shared vectors in
`shared-test-vectors/scale-amount.json`. Calorie arithmetic remains server-side.
Both clients reject custom scales above the endpoint's 1,000,000 limit before
changing the displayed scale. Invalid web URL scales use the original 1× recipe.
`shared-test-vectors/recipe-scale-validation.json` pins accepted bounds across
the Rust estimator and both clients.
