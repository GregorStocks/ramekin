# Ingredient catalog

One place that turns a written ingredient name ("softened butter", "Dry  White
Wine") into a known food. Density lookup (`volume_to_weight.rs`) and calorie
estimates (`nutrition/`) both go through it, so a name the catalog learns helps
both. The shopping-list categorizer (`ingredient_categorizer.rs`) uses the
catalog's category (`curated.json` `categories`, or a product's own) whenever
the name resolves to a categorized entry or ambiguous name. Its keyword rules
in `data/ingredients.json` cover only the names the catalog doesn't know; see
`issues/*catalog-delete-keyword-categorizer*` for when they can go.

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
   "chopped", "fresh", "large", "boneless", "baking", "raw", "unsalted", …) dropped one word at a time, so
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
   first-listed alternative that names a food (`Via::Alternative`; owner
   decision 2026-10-02), however much the alternatives differ in calories.
   Only if none does do steps 4–6 run.
   - **Pieces.** Each "or" chunk's comma pieces are tried before the chunk
     itself ("melted, unsalted butter, olive oil, or ghee" is unsalted butter;
     "apple, grape, or cranberry juice" is apple juice, not the apple that
     clause trimming in step 4 would find). Each is first cleaned of a leading
     label ("berries: sliced strawberries"), an example marker ("like cream
     cheese", "such as …"), and prep words joined by "and" ("cooked and cooled
     white rice" is cooked white rice).
   - **The list's noun.** An alternative may borrow the trailing words of the
     nearest later chunk that has some ("chicken, vegetable or seafood broth or
     stock" is chicken broth), longest first ("corn or flour tortillas" is corn
     tortillas, "sherry or red wine vinegar" sherry vinegar), or a head noun
     written first ("oil canola, olive, or …" is canola oil). A chunk lends
     nothing past a trailing clause ("grape tomatoes, sliced" lends
     "tomatoes"), when it carries its own amount ("vanilla or half a vanilla
     bean"; an article is not an amount: "peanut or a vegetable oil" is peanut
     oil), or when it is a list before the last chunk ("mushroom, vegetable,
     chicken, or beef broth" lends no "mushroom"). A one-word alternative
     (prep and state words aside) is an adjective, so it borrows first ("fresh
     lemon or lime juice" is lemon juice; "cooked white, brown, or cilantro
     lime rice" is cooked white rice). A longer one borrows first only past a
     one-word qualifier ("white wine or champagne vinegar", "or other mild
     vinegar") or one ending like itself ("red wine or white wine vinegar");
     past a distinct qualifier it is tried as written first ("white wine or
     plain white vinegar" is white wine, while "dutch process or special dark
     cocoa powder", naming nothing alone, is still dutch process cocoa
     powder). "cooled" is dropped only when joined to "cooked", since alone it
     still means cooked.
   - **An unknown first alternative.** If none of the first alternative's
     candidates resolve, its own trailing words may ("local honey or maple
     syrup" is honey; "mixed cherry or grape tomatoes" is cherry tomatoes),
     and a one-word alternative may give way to the last chunk's whole noun
     ("hot or mild paprika" is paprika; "country or sourdough bread" is
     bread). If the words dropped to get there change the food ("cooked red
     lentils or cannellini beans", "full-fat greek yogurt"), nothing counts:
     a later alternative would misreport the first. Later alternatives are
     tried only when the first names no known food at all ("quark or cream
     cheese" is cream cheese).
   - **Labels.** The calorie breakdown says what it assumed ("~120 kcal
     (assumed unsalted butter)"), including for an alternatives name a curated
     alias resolves ("butter or margarine").

Calorie estimates and density resolve whole lines with `resolve_line(item, note)`.
The parser keeps "cooked" in the note ("brown rice, cooked" → item "brown rice"),
and cooked grains, pasta, and meats differ from dry or raw ones about threefold.
Only a note that is exactly "cooked" (or "leftover cooked"), optionally followed
by a parenthetical clarifier ("cooked (about 1 cup uncooked)"), states the measured
food is cooked and tries "cooked <item>" first. If the catalog has no cooked form, the line
stays unresolved rather than being charged as the raw food (unless the item
already names a cooked food). Anything longer ("cooked and
crumbled", "cooked, drained, and cut") is a cooking instruction for a raw or dry
measure. A note naming the beverage form of a milk ("carton", "beverage",
"refrigerated kind") tries "<item> beverage" first, so "coconut milk
(refrigerated kind, such as Silk)" is the carton drink rather than the canned
default. Brand names and a bare "refrigerated" don't count: a can of coconut
milk is often "refrigerated overnight". Oil listed "for frying" in a
deep-frying amount (over about 500 kcal, roughly 1/4 cup) is a cooking medium
that is mostly discarded, so the calorie estimate reports it as unknown instead of charging the full amount. A spoonful for
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
lemon). A sized piece with no exact portion is the piece ("large lemon" →
"lemon"). A piece with sizes uses its medium ("1 stalk celery" → "stalk medium").

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
  "food_overrides": {
    "egg, whole, raw, fresh": { "default_portion": "large" },
    "thyme, fresh": { "trace_ok": true }
  },
  "categories": {
    "diamond crystal kosher salt": "Spices & Seasonings",
    "onions, raw": "Produce",
    "cheese": "Cheese"
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
  - `category` is only for products (below); foods take theirs from
    `categories`.
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

  A key that a measured line keeps whole ("1 cup medium grain rice") counts as
  already parsed, even if the bare key would lose a leading size or count. A
  key that a hand-typed shopping-list item in
  `data/shopping-list-categories.json` uses verbatim stays too, because the
  shopping list matches typed text without parsing it. A removed or re-keyed
  name takes its `categories` entry with it.

  The `curated_names_are_what_the_parser_produces` test fails until it has
  been run. The first run (2026-09-29, alongside the parser fix) removed 571
  keys, re-keyed 74, and left 131 conflicts. A review on 2026-10-06 settled the
  75 conflicts left after the next parser fix. 13 were live keys that the
  measured-line check above now recognizes. Most of the rest were deleted,
  leaving their clean name's mapping in charge, even where the split-off note
  mattered ("apples, peeled", "lemon, juiced", "chickpeas, rinsed"; see
  `issues/*note-aware-catalog-resolution*`). Seven were re-keyed by hand to
  the name the parser really stores ("baking mix", "dried mushrooms"). The one
  conflict left, "turkish or 1/2 california bay leaf", is a live key: recipes
  capitalize "Turkish", which keeps "or 1/2" in the name, and the lowercase key
  loses that.
  Recipes already stored keep the items an older parser gave them. After that
  parser fix, a one-off re-parse (since removed) re-read every stored item and
  saved a new version (source "reparse") for recipes that changed.
- `not_food` lists phrases that are not ingredients at all, with the reason.
  Names ending in ":" are headers and need no entry.
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
- `categories` gives foods and ambiguous names their shopping-list category
  (one of `ingredient_categorizer::CATEGORIES`), which beats the keyword
  rules. Keys are a curated food entry id, a USDA stripped name or unique
  description, or an ambiguous name (`null` alias) — never an alias of a
  food or a product, and at most one key per entry. Every written name that
  resolves to the entry shares its category, so judge the food as bought (the
  frozen, canned and dried forms are separate USDA foods). A compound line
  takes its first food's category.
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

Names the committed catalog still doesn't know, or calls ambiguous, are
resolved by an LLM after save (or when an estimate is read) and stored by the
server in `ingredient_name_resolutions`, shared across accounts. `learned.rs`
is the pure half:
- `unlearned_name(item)` picks the names to learn: `Unresolved` and
  `Ambiguous`. For an ambiguous name ("cheese") the model picks the food a
  recipe most likely means from its candidates (owner decision 2026-10-02,
  "model estimates, labeled"), and estimates say "(assumed cheddar cheese)".
- `candidates(name, n)` is the lexical shortlist the model must choose from. It
  covers catalog keys that share a word and name one entry.
- `resolve_line_with(item, note, &Learned)` consults a stored answer only after
  the committed catalog says `Unresolved`. An answer is a catalog key, re-resolved
  through the catalog (`Via::Learned`), so calories, density and category all
  come from committed data.
- A real food no candidate matches can instead get an estimate
  (`LearnedTarget::Estimate`): the model's calories per 100 g and, when it
  gave them, grams per cup and per piece. It isn't a catalog entry, so
  densities, categories and `resolve_line_with` ignore it; only
  `nutrition::estimate_with` counts it (`learned_estimate`), as a stand-in
  entry labeled "(estimated calories)". An ambiguous name is never estimated.
- `categorize_with(item, &Learned)` uses the committed catalog's category
  first (an ambiguous name's own included), then a learned entry's. Otherwise
  the item's own keywords decide, then the key's, so a learned answer never
  loses a category the keywords already gave. A learned non-food is
  categorized by keywords, like a committed one.

Pending, failed and "unknown" names stay unknown (an ambiguous one stays
ambiguous). The server side (queueing on
every recipe and shopping-list save, the background worker, the scrape step
`resolve_ingredient_names`, and Settings → Ingredient recognition) is in
`server/src/ingredient_names.rs`.

The table shrinks back into committed data. `make catalog-harvest-learned
LEARNED=<export>` turns resolved entry and not-food answers into step-2
decisions (entries become aliases of their key; estimates, unknowns and
ambiguous names are left out), to verify and apply like any classification
pass (`docs/agent/catalog-classification.md`). At startup the server syncs
every row with the committed catalog (`sync_with_catalog`):
- a name the catalog now knows, harvested or classified, is marked
  `harvested` (never deleted) and keeps its last answer;
- a harvested name the catalog lost again is asked again;
- an answer whose key no longer names one entry is asked again;
- an estimate or not-food answer for a name the catalog now calls ambiguous
  is asked again, since an ambiguous name takes neither;
- an "unknown" or estimated answer (no candidate fit) is re-asked once the
  catalog offers different candidates than the ones stored with it, and
  served until the new answer replaces it.

Learned names don't add gram alternatives: those are fixed at ingest from the
committed catalog. A harvested name gets them for recipes saved after its
deploy; reaching existing recipes is `p2-compute-gram-alternatives-at-read-time`.

Weights work the same way one level down. A known food with no density, or no
piece weight for a counted unit, makes `nutrition::estimate_with` report a
`WeightKey { food: entry.id, unit }` gap ("cup" for any volume, the
`piece_unit` spelling for a counted unit, "piece" for a bare count). The server
stores model estimates in `ingredient_weight_estimates`
(`server/src/ingredient_weights.rs`), and estimates read them back as
`nutrition::Weights`, only after every measurement has failed to weigh with
catalog data. These are model numbers, so the line is labeled "estimated
weight"; good ones belong in `bespoke.json` or a curated `grams_per_cup`.

## Classification passes

Aliases beyond the hand-curated core come from Claude Code subagent passes over
the names the catalog didn't resolve. The procedure, and how to rerun it, is in
`docs/agent/catalog-classification.md`.

| Date | Tier | Model / harness | Applied | Skipped |
| --- | --- | --- | --- | --- |
| 2026-09-28 | Top 2,000 unresolved names (seen ≥ 4 times across pipeline and Paprika fixtures and prod) | Claude Opus 5.5 via a Claude Code Workflow: 8 classifiers + 3 verifiers | 1,670 aliases, 16 ambiguous, 6 not-food, 4 products, 1 entry | 303 |
| 2026-09-29 | Tier 2: every remaining name seen ≥ 2 times, plus every prod name, minus tier-1 skips (2,568 names, 5,836 lines) | Claude Opus 5.5 via a Claude Code Workflow: 11 classifiers + 3 verifiers | 1,884 aliases, 128 ambiguous, 112 not-food, 4 products | 440 |
| 2026-10-01 | Secondary sources: earlier skips still unresolved, prod names the step-3 resolver answered "unknown", and every unresolved prod name (1,099 names, 3,492 lines) | Claude Opus 5.5 via a Claude Code Workflow: 6 classifiers + 3 verifiers | 487 aliases, 83 cited Branded foods, 28 FNDDS entries, 64 ambiguous, 4 not-food, 11 products | 422 |
| 2026-10-03 | Shopping categories: every uncategorized food entry or ambiguous name the fixtures, shopping-list corpus or curated names reach (1,095) | Claude Opus 5.5 via a Claude Code Workflow: 4 classifiers + 2 verifiers | 1,091 categories | 4 |

- **Shopping categories (2026-10-03):** one verifier read all 173 departures from the keyword
  rules plus a 1/7 sample of the rest, the other checked consistency; 11 corrections. All
  dried coconut is Baking. The shopping-list labels then moved canned fish, clams, canned
  meat and edamame back to Meat & Seafood and Produce. Lines categorized went from 98.2% to
  98.8% (pipeline fixtures), 98.3% to 99.4% (Paprika), and 98.9% to 99.8% (snapshots); the
  shopping-list scorecard stayed at 2 mismatches and 3 Other.
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
