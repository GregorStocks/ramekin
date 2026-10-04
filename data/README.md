# Data Directory

This directory contains test data and generated files.

## test-urls.json

A generated list of top recipe sites and their URLs, used for testing the recipe scraping pipeline.

### Generating the file

```bash
# Generate with defaults (50 sites, 10 URLs per site)
make generate-test-urls

# Or with custom options
cd cli && cargo run -- generate-test-urls --num-sites 50 --urls-per-site 20

# Merge with existing file (incremental updates)
cd cli && cargo run -- generate-test-urls --merge
```

### CLI Options

| Option | Default | Description |
|--------|---------|-------------|
| `--output`, `-o` | `data/test-urls.json` | Output file path |
| `--num-sites` | 50 | Number of sites to include |
| `--urls-per-site` | 20 | Target number of URLs per site |
| `--merge` | false | Merge with existing file instead of replacing |

### How it works

1. **Site rankings**: Fetches top food blogs from [detailed.com/food-blogs](https://detailed.com/food-blogs/), with a fallback to a hardcoded list of ~48 known recipe sites.

2. **URL discovery**: For each site, tries (in order):
   - Parse `sitemap.xml` (handles both urlset and sitemap indexes)
   - Check `robots.txt` for sitemap location
   - Scrape homepage for recipe links

3. **Recipe URL filtering**: URLs are filtered to likely recipe pages based on patterns like `/recipe/`, `/recipes/`, date-based paths (`/2025/01/`), etc.

4. **Rate limiting**: 1 second delay between sites, 200ms between sub-sitemap fetches.

### Output format

```json
{
  "generated_at": "2026-01-08T00:41:07.348374+00:00",
  "config": {
    "num_sites": 50,
    "urls_per_site": 20
  },
  "sites": [
    {
      "domain": "smittenkitchen.com",
      "rank": 10,
      "urls": [
        "https://smittenkitchen.com/2025/12/winter-cabbage-salad/",
        "..."
      ],
      "source": "sitemap"
    },
    {
      "domain": "food52.com",
      "rank": 2,
      "urls": [],
      "error": "Could not find recipe URLs from sitemap or homepage",
      "source": "failed"
    }
  ]
}
```

### Source values

- `sitemap`: URLs found via sitemap.xml
- `homepage`: URLs found by scraping the homepage
- `merged`: URLs from both a previous run and current run (when using `--merge`)
- `failed`: Could not find URLs from any source

### Merge mode

With `--merge`, the tool:
- Reads the existing file first
- Skips sites that already have enough URLs
- Unions new URLs with existing ones (deduped)
- Preserves sites from previous runs even if rankings changed

This lets you incrementally grow the dataset over time without re-scraping everything.

## shopping-list-categories.json

A small corpus of real shopping-list usage from the prod server, used to score the
ingredient categorizer (`ramekin-core/src/ingredient_categorizer.rs`) against
hand-typed items — including non-recipe store items (household goods, snacks) that
never appear in `unique-ingredients.txt`.

Each entry is one distinct item ever added to the prod `shopping_list_items` table
(including soft-deleted rows), ordered by count descending:

```json
{
  "item": "onion",       // the item string exactly as entered
  "count": 11,           // how many times it was added on prod
  "category": "Produce"  // the expected grocery-aisle category (hand/LLM-labeled)
}
```

`make shopping-list-categorizer-test` runs the categorizer over the labeled corpus
and reports accuracy, the mismatches, and the "Other" rate — both per distinct item
and weighted by usage count (`ramekin-core/tests/shopping_list_categorizer_tests.rs`),
so a regression on frequently-added items can't hide. The follow-up issue
`p2-expand-ingredient-categorizer-keywords` mines this corpus to expand
`data/ingredients.json`.

### Regenerating the corpus

The corpus is a deliberate one-off, read-only extraction from the prod database. It
is intentionally **not** a Makefile target and the extraction query is intentionally
**not** checked into the repo — it needs prod access, and per `AGENTS.md` a one-off
prod query should be confirmed with the maintainer rather than baked into the
codebase. To refresh it, ask Gregor to run a read-only extraction of the distinct
`shopping_list_items` (grouped by item with usage counts, including soft-deleted
rows, ordered by count descending), merge the results into
`data/shopping-list-categories.json`, and label any new items so
`make shopping-list-categorizer-test` passes.

## ingredient-catalog-audit.md

Coverage report for the ingredient-name matchers: nutrition (`nutrition::estimate`),
density (`catalog::grams_per_cup`), and shopping categories (`categorize`). It shows how much
of each corpus is recognized and the nutrition failure reasons. It is the measuring
stick for the ingredient catalog work (`issues/*ingredient-catalog*`): a matcher
change should show up as a diff here.

`make ingredient-catalog-audit` regenerates it from committed corpora only, so it is
deterministic:

- `ramekin-core/tests/fixtures/ingredient_parsing/pipeline/` and `.../paprika/`: one
  recipe per fixture file (curated fixtures are single-line edge cases and are skipped)
- `data/pipeline-snapshots/`: the only committed corpus with servings, so it is the
  only one reporting per-serving coverage
- `data/shopping-list-categories.json`: hand-typed shopping-list items

`make pipeline` runs it too, after regenerating the fixtures. A CLI unit test
(`committed_report_is_current`, part of `make test`) regenerates the report and fails
if the committed file is stale, so a matcher change must commit the new numbers.

Each run also writes `logs/ingredient-catalog-audit-local.md` (never committed): the
same numbers plus the most frequent unrecognized names per matcher. Those lists shift
with every catalog change, so they stay out of the committed report. Two optional
corpora are added to the local report only:

- `RUNS_DIR=path/to/data/pipeline-runs` audits the newest full pipeline run.
- `PROD_RECIPES=path/to/recipes.json` audits a prod dump: a JSON array of
  `{"servings": ..., "ingredients": [...]}` objects, where `ingredients` is the stored
  `recipe_versions.ingredients` value of each live recipe's current version. Like the
  shopping-list corpus, this is a one-off read-only extraction Gregor runs; the query
  and the dump are intentionally not checked in.
- `LEARNED=path/to/learned.json` (with `PROD_RECIPES`) applies the server's learned
  names to the prod dump, as the server does when it estimates: a JSON array of
  `ingredient_name_resolutions` rows (`name`, `status`, `disposition`, `catalog_key`,
  and an estimate's `kcal_per_100g`, `grams_per_cup`, `grams_per_piece`). Add `model`
  for `make catalog-harvest-learned`, which reads the same export.

The local report also lists the most frequent nutrition failures as "reason: name",
the quickest way to see what to fix next.
