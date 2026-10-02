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
- Per-piece weights (via the linked food's `portions`): `grams_per_piece(entry,
  unit)` converts counts ("3 eggs", "2 cloves garlic", "1 medium onion").

So "black pepper" resolves (its calories are known) but has no density, because
grind size changes the weight of a cup too much. Names that could mean several
foods ("cheese", "rice", "chicken", "yogurt") are ambiguous for every attribute.

A resolution is one of:

- `Entry`: one food or product.
  - **Products** (`kind: "product"`) are bought but not eaten: parchment paper,
    skewers. They carry only a shopping `category`, and calorie estimates skip them.
- `Compound`: several foods joined by "and" or "&" on one line with one amount
  ("salt and pepper"). "and/or" offers alternatives (below).
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
  pepper), a food with a `trace_ok` food override (fresh herbs), or the curated
  entry sets `trace_ok: true`. A line of it is negligible when it has no numeric
  amount ("to taste", no measurement), a trace unit (pinch, dash, sprig), or
  counts at most 10 small pieces (after scaling) USDA has no weight for ("2 bay
  leaves", "1 cinnamon stick"). So is at most a tablespoon (after scaling) of one
  with no density ("1 tsp freshly ground black pepper"): about 15-25 kcal for most
  spices, and up to about 50 for dense, rich seeds such as whole mustard seed. A
  real amount, like a cup of cumin, counts in full.

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
   "chopped", "fresh", "large", "boneless", "baking", …) dropped one word at a time, so
   "grated fresh ginger" tries "fresh ginger" before "ginger". Words that change
   the food ("ground", "dried", "light", "crushed", "whole") are never dropped.
   A *bare* name reached this way may not be a dried or ground spice unless the
   original said "dried" or "ground". "fresh rosemary" stays unknown rather than
   becoming dried rosemary. A curated alias that keeps a prep word ("grated
   nutmeg") is an explicit choice and still counts.
6. The name without a leading measure the parser left in it ("8 tbsp unsalted
   butter", "240 ml heavy cream", "cloves garlic", "can of tomato paste"), then
   steps 3 and 5 on that. A count or unit never names the food.
7. A name offering alternatives ("x or y", "x and/or y") that steps 1–3 don't
   match directly (exactly, as a plural, or without modifiers) resolves to the
   first-listed
   alternative that names a food (`Via::Alternative`; owner decision
   2026-10-02), however much the alternatives differ in calories. Each "or"
   chunk's comma pieces are tried before the chunk itself ("melted, unsalted
   butter, olive oil, or ghee" is unsalted butter; "apple, grape, or cranberry
   juice" is apple juice, not the apple that clause trimming in step 4 would
   find). Each alternative is first cleaned of a leading label ("berries:
   sliced strawberries"), an example marker ("like cream cheese", "such as
   …"), and prep words joined by "and" ("cooked and cooled white rice" is
   cooked white rice). Only if no alternative names a
   food do steps 4–6 run. A single word is usually an
   adjective, so every alternative is first tried with the list's noun ("fresh
   lemon or lime juice" is lemon juice), then alone: the last
   alternative's trailing words, longest first ("corn or flour tortillas" is
   corn tortillas, "sherry or red wine vinegar" sherry vinegar), unless that
   alternative carries its own amount ("vanilla or half a vanilla bean"; an
   article is not an amount: "peanut or a vegetable oil" is peanut oil), or a
   head noun written
   first ("oil canola, olive, or …" is canola oil). Longer alternatives are
   tried alone, then with the trailing words. The calorie breakdown says what
   it assumed ("~120 kcal (assumed unsalted butter)"), including for an
   alternatives name a curated alias resolves ("butter or margarine").

Calorie estimates and density resolve whole lines with `resolve_line(item, note)`.
The parser keeps "cooked" in the note ("brown rice, cooked" → item "brown rice"),
and cooked grains, pasta, and meats differ from dry or raw ones about threefold.
Only a note that is exactly "cooked" (or "leftover cooked"), optionally followed
by a parenthetical clarifier ("cooked (about 1 cup uncooked)"), states the measured
food is cooked and tries "cooked <item>" first. If the catalog has no cooked form, the line
stays unresolved rather than being charged as the raw food (unless the item
already names a cooked food). Anything longer ("cooked and
crumbled", "cooked, drained, and cut") is a cooking instruction for a raw or dry
measure. Oil listed "for frying" in a deep-frying amount (over about 500 kcal, roughly
1/4 cup) is a cooking medium that is mostly discarded, so the calorie estimate
reports it as unknown instead of charging the full amount. A spoonful for
browning still counts.

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
  `grams_per_cup`, plus `portions` (grams per piece) and `default_portion`.
- `names`: stripped name → `fdc_id`.

Density rules:

- Divide portion gram weight by amount. Prefer cup portions, then tablespoons
  (16 per cup), then teaspoons (48 per cup), then fluid ounces (8 per cup; wines,
  spirits and many juices are weighed only per fluid ounce), and average within
  the preferred unit. Only a bare "fl oz" modifier counts: servings such as
  "jigger (1.5 fl oz)" are not volumes. A dry powder's fluid-ounce portions
  (described "powder" and not "prepared") weigh the prepared drink, so they
  are skipped. A fluid-ounce density outside 150-330 g/cup fails the import
  when it is the one used.
- Accept `cup`, `cups`, `cup, ...` and `cup (...)`. Skip chip portions and other units.
- Seven zero-amount cup rows, and one fluid-ounce row whose amount is wrong (a
  "5 fl oz" infant formula row weighing 30 g), are pinned in `EXCLUDED_PORTIONS`.
  Any other malformed supported-volume portion fails the import.

Piece rules (`portions`):

- Every portion that is not a volume, mass, or package measure (cup, oz, can,
  serving, …) is a piece. Its key is the modifier lowercased, without
  parentheticals, with the first word singular ("cloves" → "clove", "leaves" →
  "leaf") and a trailing "whole" or "raw" dropped. A size after a comma is kept:
  "stalk, medium (…)" → "stalk medium".
- The weight is gram weight ÷ amount ("3 cloves" of 9 g → 3 g). The first
  portion in portion-table order wins a key. Zero-amount rows and the cooked
  yield of a pound of meat or a whole recipe are not pieces.
- `default_portion`, the piece a bare count means, is the first of medium,
  fruit, whole, large, small; else the one "<piece> medium"; else the only
  piece. Parts of a piece (slice, strip, wedge, ring, …) are never the default.

At lookup, a counted unit is normalized the same way ("extra-large" → "extra
large"). A size with no exact portion uses the food's one sized piece ("2 medium
potatoes" → "potato medium"), else its default piece ("1 large lemon" → the
lemon). A piece with sizes uses its medium ("1 stalk celery" → "stalk medium").

Name rules:

- Lowercase, collapse whitespace, and strip the trailing suffixes in `NAME_SUFFIXES`
  (", raw", ", dry", ", enriched…") in list order.
- When several foods share a stripped name, the first food with a density in
  portion-table order wins. Otherwise the first food in food-table order wins.

The importer rejects empty tables, duplicate IDs, unknown food references, and
invalid energies. Repeated imports produce identical bytes. Its offline tests
are `tests/test_catalog_import.py`.

### `data/fndds.json` (generated; do not edit)

The same `make catalog-import` also projects the pinned USDA FNDDS (survey foods)
October 2024 CSV archive (SHA-256 verified, cached at
`.cache/nutrition-fndds-2024-10-31.zip`). FNDDS is a secondary source for foods SR
Legacy lacks (guacamole, gnocchi, simple syrup, tahini, liqueur). Its records have
the same shape as `usda.json` foods:

- `kcal_per_100g` is nutrient number 208 (Energy, kcal); FNDDS keys nutrients by
  number, not id.
- `grams_per_cup` averages the volume portions whose description starts with an
  amount and a volume unit ("1 cup", "1/2 cup, diced", "1 tablespoon", "1 fl oz").
  Cups win over tablespoons, teaspoons, then fluid ounces. Portions that measure
  the food before it's eaten ("1 cup, dry, yields", "unpopped", "1 teaspoon,
  dry") or with ice are skipped, since their weight isn't the eaten food's.
- `portions` is always empty: FNDDS pieces ("1 medium", "1 slice") are not
  imported yet.

There is no name index. FNDDS foods are never entries or names on their own, so
adding the release changed no existing resolution. A curated entry reaches one by
linking its `fdc_id`; the importer and the catalog loader both reject an FDC id
that appears in both releases.

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
  "rewrites": { "salt": "kosher salt" },
  "food_overrides": {
    "egg, whole, raw, fresh": { "default_portion": "large" },
    "thyme, fresh": { "trace_ok": true }
  }
}
```

- `entries` override USDA foods of the same name.
  - `fdc_id` links the food whose calories apply: an SR Legacy food, or an FNDDS
    food from `fndds.json`.
  - `kcal_per_100g` (`{"value", "source", "url"}`) is the hand-curated fallback
    for a food neither USDA release has (e.g. garam masala). It cites a specific
    record, normally a USDA FoodData Central Branded label
    (`https://fdc.nal.usda.gov/food-details/<id>/nutrients`). An entry has either
    an `fdc_id` or cited calories, never both. The catalog loader requires
    the source and an `https://` URL. With neither, its calories stay
    unknown rather than wrong.
  - `grams_per_cup` is either a cited value, or `{"none": reason}` to suppress
    the linked food's density. Omit it to inherit the linked food's density.
  - Every value needs a `source`.
  - The 23 manual baking values still lack individual citations. See
    `issues/p3-cite-embedded-manual-density-values`.
  - `category` (one of `ingredient_categorizer::CATEGORIES`) overrides the
    keyword categorizer.
  - `trace_ok: true` marks a food commonly listed without an amount, for foods
    the "spices, …" rule doesn't cover. An entry with only `trace_ok` (no
    `fdc_id`, calories or density) is a trace-only spice or herb whose calories
    no source gives: a line of it is negligible with no amount, a trace unit,
    a few pieces, or up to a tablespoon, and unknown for anything more. Every
    food entry needs at least one of `fdc_id`, `kcal_per_100g`,
    `grams_per_cup` or `trace_ok`.
  - `portions` (cited, like `food_overrides` portions) and `default_portion`
    give a name its own pieces, replacing the linked food's: "hamburger buns"
    and "hot dog buns" share one USDA food but weigh 57 and 44 g, and a
    baguette doesn't inherit generic French bread's 139 g slice.
  - `kind: "product"` entries need a `category` and may not have an `fdc_id`,
    `kcal_per_100g`, or `grams_per_cup`.
- `aliases` map a name to an entry id, USDA stripped name, or unique USDA
  description. A `null` alias marks a name as ambiguous. An alias decides food
  identity for *every* attribute. When USDA has the food itself, point at it
  (e.g. "beef broth"). If only its density is missing, add an entry linking the
  real food (`fdc_id`) with an approximate `grams_per_cup` and a source
  explaining it (see "dried thyme", "greek yogurt"). Aliasing to a stand-in
  food is only for foods USDA lacks entirely ("dijon mustard" → yellow mustard,
  "shaoxing wine" → sake), where the stand-in supplies every attribute.
  A name offering alternatives resolves to its first-listed food (resolver step
  7), so "x or y" aliases are only needed when that would pick the wrong food;
  older `null` aliases for dissimilar alternatives ("heavy cream or milk") are
  now overridden by step 7. Bare herb
  names follow how recipes use them: "rosemary" and "ginger" mean fresh; "sage"
  is ambiguous because USDA has no fresh sage. Bone-in cuts and whole birds are
  `null` too: recipes give their purchased weight, bones included, while USDA
  describes only the edible part, so a 4 lb whole chicken would be charged as
  4 lb of meat. Boneless cuts map normally.
- Alias and not-food keys are what the parser stores as `item`, never raw line
  text. A key with an amount, a prep clause or a usage note ("about 7 cloves
  garlic", "oil, for frying") can never match a newly parsed line.
  `make catalog-clean-aliases` re-parses every key:
  - it removes the key when the parsed name already resolves the same way;
  - it re-keys it to the parsed name when that name resolves to nothing, but
    only when the re-parse split off nothing but notes;
  - it reports conflicts to `logs/catalog-alias-cleanup.md` and leaves them
    alone.

  A re-key counts as a conflict if the re-parse would:
  - drop an example or alternative ("fresh herbs such as basil" must not make
    "fresh herbs" mean basil);
  - drop an amount, meaning the key was a fragment of a line;
  - leave a fragment ("pepper or");
  - move an ambiguous (null) alias onto a shorter name.

  The `curated_names_are_what_the_parser_produces` test fails until it has
  been run. The first run (2026-09-29, alongside the parser fix) removed 571
  keys, re-keyed 74, and left 131 conflicts.
  Recipes already stored keep the items an older parser gave them. After that
  parser fix, a one-off re-parse (since removed) re-read every stored item and
  saved a new version (source "reparse") for recipes that changed.
- `not_food` lists phrases that are not ingredients at all, with the reason.
  Names ending in ":" are headers and need no entry.
- `rewrites` rename the stored ingredient at import ("salt" → "kosher salt").
- `food_overrides` correct SR Legacy foods by their unique description:
  - `default_portion` picks the piece a bare count means: "3 eggs" are large,
    the US recipe convention, and "4 strips bacon" are slices, although the
    importer never makes a slice the default;
  - `portions` adds cited pieces the release doesn't weigh
    (`{"head": {"value": 24, "source": ..., "url": ...}}` for garlic), keyed like
    imported pieces;
  - `trace_ok` marks fresh herbs, which recipes list by the sprig.
  FNDDS foods are reached through curated entries, so their corrections live on
  those entries.
- Piece sources, in order: the food's own USDA portions, then USDA FNDDS
  portions for the same food, the Canadian Nutrient File's household measures,
  or a USDA Branded label, in `curated.json`; then `bespoke.json` (below).

### `data/bespoke.json` (hand-maintained)

Piece weights no published database gives, each with the basis for its number:

```json
{
  "pieces": {
    "shallots": {
      "pieces": { "medium": { "grams": 28, "basis": "Medium shallot: 1 ounce (America's Test Kitchen)", "url": "https://..." } },
      "default": "medium"
    },
    "black beans": { "pieces": { "can": { "grams": 425, "basis": "Standard US can of beans: 15 oz" } } }
  }
}
```

- Keys are any catalog name; the pieces are added to the entry the name
  resolves to (on top of its own or its food's), so every alias of that food
  gets them.
- `basis` says where the number comes from: a reference's statement (cooking
  references such as America's Test Kitchen, produce weight tables, a product
  listing), a stated convention, or arithmetic on those ("twice a 1-inch
  piece"). `url` is optional but expected when there is a reference.
- Bespoke pieces never shadow a published piece for the same food, and
  `default` (the piece a bare count means) applies only when the entry has
  none.
- A bare "can" uses the food's standard US can size (owner decision,
  2026-10-02): 15 oz for beans, 14.5 oz for diced or whole tomatoes, 6 oz for
  tomato paste, and so on. A food sold in several common sizes (crushed
  tomatoes) has none and stays unknown.

The loader asserts that keys are normalized, targets exist, aliases don't
shadow names, and densities are finite and positive. `catalog::version()`
hashes both files and the rule version; bump `RULE_VERSION` in `mod.rs` when
resolution behavior changes.

## Learned names (catalog step 3)

Names the committed catalog still doesn't know are resolved by an LLM after
save and stored by the server in `ingredient_name_resolutions`, shared across
accounts. `learned.rs` is the pure half:
- `unlearned_name(item)` picks the names to learn: `Unresolved` only.
  Deliberately ambiguous names ("cheese") are never sent.
- `candidates(name, n)` is the lexical shortlist the model must choose from. It
  covers catalog keys that share a word and name one entry.
- `resolve_line_with(item, note, &Learned)` consults a stored answer only after
  the committed catalog says `Unresolved`. An answer is a catalog key, re-resolved
  through the catalog (`Via::Learned`), so calories, density and category all
  come from committed data. A model never supplies numbers.
- `categorize_with(item, &Learned)` uses a learned entry's catalog category when
  it has one. Otherwise the item's own keywords decide, then the key's, so a
  learned answer never loses a category the keywords already gave. A learned
  non-food is categorized by keywords, like a committed one.

Pending, failed and "unknown" names stay unknown. The server side (queueing on
every recipe and shopping-list save, the background worker, the scrape step
`resolve_ingredient_names`, and Settings → Ingredient recognition) is in
`server/src/ingredient_names.rs`. Harvesting good answers back into
`curated.json` is `p2-catalog-harvest-learned-names`.

## Classification passes

Aliases beyond the hand-curated core come from Claude Code subagent passes over
the names the catalog didn't resolve. The procedure, and how to rerun it, is in
`docs/agent/catalog-classification.md`.

| Date | Tier | Model / harness | Applied | Skipped |
| --- | --- | --- | --- | --- |
| 2026-09-28 | Top 2,000 unresolved names (seen ≥ 4 times across pipeline and Paprika fixtures and prod) | Claude Opus 5.5 via a Claude Code Workflow: 8 classifiers + 3 verifiers | 1,670 aliases, 16 ambiguous, 6 not-food, 4 products, 1 entry | 303 |
| 2026-09-29 | Tier 2: every remaining name seen ≥ 2 times, plus every prod name, minus tier-1 skips (2,568 names, 5,836 lines) | Claude Opus 5.5 via a Claude Code Workflow: 11 classifiers + 3 verifiers | 1,884 aliases, 128 ambiguous, 112 not-food, 4 products | 440 |
| 2026-10-01 | Secondary sources: earlier skips still unresolved, prod names the step-3 resolver answered "unknown", and every unresolved prod name (1,099 names, 3,492 lines) | Claude Opus 5.5 via a Claude Code Workflow: 6 classifiers + 3 verifiers | 487 aliases, 83 cited Branded foods, 28 FNDDS entries, 64 ambiguous, 4 not-food, 11 products | 422 |

- **Verification:** the verifiers checked every mapping to a ≥ 300 kcal/100 g food (543), a
  ~1/7 sample of the rest, and consistency across all decisions. They corrected 34 decisions.
  On review, 8 generic dish names ("soup", "meatballs") were changed from not-food to skip,
  so a real "1 lb frozen meatballs" line is never silently dropped.
- **Pipeline-fixture effect:**
  - recognized names on food lines went from 54.0% to 77.8%;
  - calories computed from 44.7% to 58.3%;
  - volume lines with a density from 62.9% to 77.5%.
- **Tier 2 verification:** the verifiers checked 360 high-calorie mappings, a 262-name
  sample, and consistency across 1,400 decisions. They corrected 28 decisions, mostly guesses
  turned into skips: panko (half the density of dry crumbs), and blends USDA lacks such as
  shichimi, Tajín and Italian seasoning.
- **Tier 2 not-food review:** all 113 not-food names were read by hand. The remaining 112 are
  blog "years ago:" links, yield and serving lines, section headers, and parser fragments.
- **Tier 2 PR review:** 36 names became ambiguous.
  - An "alternate fillings" line was not-food, but it names real food.
  - "spring roll wrappers" was aliased to wonton wrappers, but it covers both rice paper and
    wheat pastry.
  - A second 2-agent pass re-checked all 484 "x or y" aliases against a numeric rule. It
    allows alternatives within about 15% in calories per the recipe's measure, trace amounts,
    or names that aren't really alternatives. It flagged 34: butter or oil, honey or sugar,
    sake or white wine, sour cream or crème fraîche, and powdered or granulated sugar by
    volume.
- **Tier 2 effect:**

  | Measure | Prod (992 recipes) | Pipeline fixtures |
  | --- | --- | --- |
  | Names recognized | 79.8% → 91.6% | 77.7% → 82.5% |
  | Calories computed | 69.9% → 79.2% | 70.6% → 74.4% |
  | Estimates complete | 5.9% → 14.5% | 7.3% → 10.4% |
  | "Not enough data" | 44.7% → 25.8% | 40.3% → 32.7% |
- **Secondary-source pass (2026-10-01):**
  - `fndds.json` and cited Branded labels (see "Data files") supplied the foods SR Legacy
    lacks: panko, crème fraîche, gochujang, pancetta, guanciale, chili crisp, coconut
    sugar, tapioca and cassava flour, and others.
  - Every cited record was checked mechanically against the downloaded USDA Branded release
    (2026-04-30). Each must exist with exactly the cited kcal per 100 g and a gram serving
    of at least 10 g, and its density must match the household serving.
  - The verifiers made 153 corrections: outlier records replaced with typical ones,
    duplicate foods merged into one food plus aliases, and names moved to SR Legacy or
    FNDDS when those came first in the source order. Two resulting alias loops (chipotle in
    adobo, sweet chili sauce) were fixed by hand.
  - `make catalog-clean-aliases` then removed 348 older keys the parser now splits.
    Their parsed names resolve on their own. Only stored items in accounts that were never
    re-parsed still used them (351 lines; none in the owner's account).
  - On review, 46 not-food decisions became skips. Dish and sauce names ("meatballs",
    "teriyaki glaze") can be real food, and parser fragments ("2 tablespoons", "pitted")
    stand for a food the parser lost. Not-food would drop their calories silently.
    `dish_names_are_never_not_food` guards the common dish names.
  - The "x or y" aliases were re-checked against the 15% rule. Six became ambiguous:
    simple syrup or maple syrup, dry vermouth or white wine (both orders), amaretto,
    kahlua or brandy, butter or oils, and bacon or pancetta (now that pancetta has a
    value). Buffalo wing sauce was dropped: labels range from near-zero hot sauce to
    butter-based restaurant sauce.
  - Of the skips, 94 names (102 prod lines) are spices whose labels only have servings
    under 10 g, which can't be cited. A follow-up made 52 of them, plus fresh herbs listed
    by the leaf, trace-only entries (with 100 aliases for their variants): sumac, celery salt, za'atar, five
    spice, Old Bay, bitters, liquid smoke, vanilla beans, MSG, bonito flakes, fresh sage
    and tarragon. Whole spices whose ground form USDA has instead link it, since grinding
    changes the volume weight but not the calories by weight: mustard seeds and allspice
    berries (density marked unknown, like black pepper). "Japanese chili powder"
    aliases shichimi togarashi, which is what recipes mean by it. Fresh herbs stay trace-only: dried
    forms have several times the calories per gram.
  - Effect:

    | Measure | Prod (owner's 499 recipes, with learned names) | Pipeline fixtures |
    | --- | --- | --- |
    | Names recognized | 93.3% → 95.0% | 83.3% → 85.2% |
    | Calories computed | 83.2% → 84.4% | 76.9% → 78.3% |
    | Estimates complete | 21.0% → 22.4% | 12.8% → 14.5% |
    | "No nutrition match" lines | 304 → 157 | 9,499 → 8,261 |
