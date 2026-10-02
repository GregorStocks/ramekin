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
  "Heaped", "heaping", "scant", "generous", "level" and "rounded" volumes count
  as the plain unit.
- Packages that state their weight ("15-ounce can", "(28-oz.) can", "14
  1/2-ounce can", "12- to 18-ounce package", "425-gram package") use that weight for any food, including a weight the parser moved
  into the note ("2 (15-ounce) cans" is unit "can", note "15-ounce, drained").
  A can with no stated weight is unknown.
- Counts use USDA per-piece weights (catalog README, "Piece rules"): "3 eggs"
  (large, 50 g each), "2 cloves garlic" (3 g), "1 medium onion" (110 g), "1
  stalk celery" (40 g), "1 1/2 sticks butter" (113 g each). A count the food
  has no piece weight for ("1 head garlic") is unknown.
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
results, except that a name the catalog doesn't know can change from unknown to
counted once its learned answer arrives (see "Names the catalog doesn't know").
Numeric fields retain calculation precision; display strings round as
described under Presentation.

## Names the catalog doesn't know

After a recipe or shopping-list item is saved, names the committed catalog
doesn't know are resolved by an LLM in the background (catalog step 3). The
answer is a catalog key or "not food", stored once per name and used by later
estimates. Saving never waits for it, and estimating never calls the LLM:
- a name not resolved yet reads as unknown ("Not recognized");
- so does a name the model couldn't place;
- a failed resolution shows in Settings → Ingredient recognition, where it can
  be retried.

While any of an estimate's names is still pending, the response sets
`resolving`. The web and iOS clients then re-request the same estimate every
2 seconds, keeping the current one on screen, until it clears.

## Negligible, compound, and non-food lines

Some lines are known without a usable amount (rules in the catalog README):

- **Zero-calorie foods** (salt, water, baking soda) contribute 0 kcal on any line.
- **Spices** (USDA "spices, …", including black pepper) and fresh herbs
  contribute 0 kcal when the line has no numeric amount ("to taste"), only a
  pinch, dash, or sprig, counts at most 10 small pieces (after scaling) USDA
  has no weight for ("2 bay leaves", "1 cinnamon stick"), or is at most a
  tablespoon (after scaling) of one with no density ("1 tsp freshly ground black
  pepper"); more is unknown. A real amount is computed normally.
- **Compound lines** ("salt and pepper", one amount for several foods) contribute
  0 kcal only if every food is negligible on that line. Otherwise they are
  unknown with "Several ingredients share one amount", because one amount can't
  be split between foods.
- **Leftover section headers** (names ending in ":"), serving notes ("to serve")
  and products such as parchment paper are skipped. They add nothing and are not
  listed as unknown. A recipe of only such lines is "No ingredients to estimate."

## Lines with no amount

A line given no amount at all ("olive oil", "lime wedges, to serve", "butter, for
the pan") has nothing to count, so it is left out rather than unknown: its
breakdown text is "No amount given" and it is listed under "Not counted", but it
never makes an estimate partial or insufficient. A recipe whose other lines all
count is complete, with its figure and "Not counted: olive oil" under it. If
nothing else was counted, or only a trace under 1 kcal (salt), there is no
figure to show and the estimate is insufficient. An unclear amount ("a knob of
butter") is not this: it stays unknown ("Amount unclear").

## Presentation

The server decides everything and formats every string; web and iOS only lay
them out. Each estimate has a `status`, set by how many real ingredients it
couldn't count (negligible lines, non-food lines, and lines given no amount
never count against it):

| Uncounted | Status | Headline |
| --- | --- | --- |
| 0 | `complete` | "~780 kcal per serving", or "~3,100 kcal for the whole recipe" without servings; "Not counted: olive oil" if lines had no amount |
| 1–3 | `partial` | "At least ~780 kcal per serving", plus "Not counted: mirin, garlic" |
| 4+, or nothing counted | `insufficient` | "Not enough ingredient data to estimate calories" |
| no estimable lines | `empty` | "No ingredients to estimate" |

A per-serving headline has the whole-recipe figure as its `secondary` line. A
partial estimate shows only its lower bound, and an insufficient one shows no
number at all. Numbers use whole calories under 100 and tens above, with
thousands separators ("~97", "~3,100"). A single estimate rounds to the nearest
value. Range ends round outward ("~380–780 kcal" for 387–774), and a lower bound
rounds down ("At least ~380"), so the display never claims more than was counted.
A lower bound under 1 kcal is insufficient rather than partial.

The cutoff of 3 uncounted ingredients (`nutrition::MAX_UNKNOWN_LINES`) came from
the "Uncounted ingredients per recipe" table in the catalog audit, run against
prod on 2026-09-28. With 992 recipes, a cutoff of 2 would show a number for only
40% of them, 3 shows one for 56%, and 5 for 82%. However, a lower bound missing
five ingredients can be far below the real total. The cutoff should rise only if
the audit shows most uncounted lines are minor.

A "How is this calculated?" disclosure lists every line with its scaled calories,
"Negligible", "Not a food", "No amount given", or a short reason it couldn't be counted ("Not
recognized", "Could be several foods", "Amount unclear", "Can't convert this
measurement to weight"), followed by the USDA attribution.

A per-serving estimate needs a positive serving count or range.
- **Prefixes:** `serves`, `servings`, `yield`, `yields` or `makes`, with or without a colon (case-insensitive).
- **Suffixes:** `serving(s)`, `people`, `person(s)` or `portion(s)`. A "makes" count needs one ("Makes 4 servings"); a bare "Makes 24" is usually cookies.
- **Examples:** "4", "Serves 4 to 6", "Servings 2", "Yield: 4", "Makes 4 servings".
- **Ranges:** a range gives a per-serving range, from the total over the most servings to the total over the fewest. "4 to 6 servings" of 2,400 kcal is ~400–600 kcal.
- **Not counted:** a yield of something other than servings ("Makes 12 cookies", "1 loaf").

The web and iOS scalers accept the same prefixes when scaling the displayed servings (pinned by `shared-test-vectors/scale-amount.json`). Scaling multiplies both ingredients and servings, so per-serving calories stay the same.
Partial per-serving estimates are lower bounds as well. Imported nutrition
text is displayed separately and never feeds the calculation.

Web and iOS also scale displayed ranges, mixed numbers, compound measurements,
and explicit serving-count labels. This presentation logic remains duplicated
in their existing scale helpers and is pinned by shared vectors in
`shared-test-vectors/scale-amount.json`. Calorie arithmetic remains server-side.
Both clients reject custom scales above the endpoint's 1,000,000 limit before
changing the displayed scale. Invalid web URL scales use the original 1× recipe.
`shared-test-vectors/recipe-scale-validation.json` pins accepted bounds across
the Rust estimator and both clients.
