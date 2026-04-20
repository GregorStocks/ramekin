# Pipeline Snapshots — Design

## Problem

When we change extraction, ingredient parsing, or auto-tagging logic, we have no easy way to see the resulting change on specific recipes. `make pipeline` writes detailed per-step outputs to `data/pipeline-runs/<timestamp>/…`, but that directory is gitignored and the contents differ per run (timestamps, UUIDs, ordering), so diffs aren't useful. Reviewers need to be able to see "here's what this recipe looked like before, here's what it looks like after" for an allowlisted set of URLs.

The smitten-kitchen sizzling chicken fajitas recipe is the immediate motivating example — its ingredient section headings aren't being recognized and we want a low-friction way to iterate on that and see the result committed in the diff.

## Observation: consolidated recipe state never gets materialized

The CLI's `SaveRecipeStep` (`cli/src/pipeline/steps.rs:156-211`) emits `{ raw_recipe, saved_at }` — just echoing the pre-parse blob from `extract_recipe`. It ignores `parse_ingredients`. The server's `SaveRecipeStep` (`server/src/scraping/steps.rs:267-380`) consolidates raw_recipe + parsed_ingredients + image ids into the DB write but returns only `{ recipe_id }` as its step output.

Neither has access to tags at `save_recipe` time because the step order in both pipelines is `save_recipe → enrich_auto_tag → apply_auto_tags → enrich_generate_photo`. So tags exist only in later step outputs.

The snapshot therefore has to be assembled at pipeline-end by reading multiple step outputs, not by copying a single step's output. We'll put the assembly logic in a shared core helper so the CLI snapshot writer and the server status endpoint can both use it.

## Design

### 1. Shared consolidation in `ramekin-core`

Add a new module `ramekin-core/src/final_recipe.rs` exposing:

```rust
pub struct FinalRecipe {
    pub title: String,
    pub description: Option<String>,
    pub servings: Option<String>,
    pub prep_time: Option<String>,
    pub cook_time: Option<String>,
    pub total_time: Option<String>,
    pub instructions: String,
    pub image_urls: Vec<String>,
    pub source_url: Option<String>,
    pub source_name: Option<String>,
    pub ingredients: Vec<ParsedIngredient>,
    /// Tags that were actually applied (present when apply_auto_tags succeeded).
    pub applied_tags: Option<Vec<String>>,
    /// Tags suggested by enrich_auto_tag (present even when apply_auto_tags didn't run).
    pub suggested_tags: Option<Vec<String>>,
}

pub fn build_final_recipe(
    raw_recipe: &RawRecipe,
    parsed_ingredients: Option<&[ParsedIngredient]>,
    suggested_tags: Option<&[String]>,
    applied_tags: Option<&[String]>,
) -> FinalRecipe;
```

- Fields from `raw_recipe` are copied.
- `ingredients` comes from `parsed_ingredients` when present; when absent/empty, falls back to line-splitting `raw_recipe.ingredients` into `ParsedIngredient` stubs (matching the existing fallback in the server's `SaveRecipeStep` at `server/src/scraping/steps.rs:334-353`).
- `applied_tags` and `suggested_tags` are both optional; callers pass whichever are available.
- Serialization uses `#[serde(skip_serializing_if = "Option::is_none")]` so missing fields don't pollute diffs.
- No changes to `SaveRecipeStep` in either CLI or server as part of this work.

### 2. Snapshot allowlist

- New file `data/pipeline-snapshot-urls.json`:
  ```json
  [
    "https://smittenkitchen.com/2014/03/sizzling-chicken-fajitas/"
  ]
  ```
- Add that URL to `data/test-urls.json` so it's part of the normal pipeline corpus.

### 3. Snapshot writer

- New module `cli/src/pipeline/snapshots.rs` with `write_snapshots(run_dir, allowlist_path, snapshots_dir) -> Result<()>`.
- Called from `pipeline_orchestrator.rs` after the run completes, before the final manifest write.
- For each URL in the allowlist:
  1. Compute its `slugify_url` and locate `run_dir/urls/<slug>/`.
  2. If the URL directory doesn't exist → **error and abort** (URL wasn't in the run's URL set).
  3. Read `extract_recipe/output.json` (required), `parse_ingredients/output.json` (optional), `enrich_auto_tag/output.json` (optional), and `apply_auto_tags/output.json` (optional) from that directory.
  4. Call `build_final_recipe(...)` to produce a `FinalRecipe`.
  5. Serialize to pretty JSON and write to `data/pipeline-snapshots/<slug>.json`.
- If `extract_recipe` output is missing → error (the pipeline wouldn't have gotten far enough for a meaningful snapshot). Missing later-step outputs are fine — those fields are just `None` in the snapshot.
- `data/pipeline-snapshots/` is created if needed. Not gitignored.

### 4. Image URL determinism

For now, image URLs are kept verbatim in the snapshot. The HTML cache (`data/pipeline-cache/`) means re-runs hit the same cached HTML and produce the same URLs. If we later observe real drift (sites embedding timestamps or cache-busters in image URLs), we'll add canonicalization — but YAGNI until we see it.

`fetch_images` step output (fetched/failed network state) is never part of the snapshot, only the extracted `image_urls` from `raw_recipe`.

## Non-goals

- **Not** building an ad-hoc "scrape one URL, print result" CLI. `cli parse-html <file> --source-url <url>` already covers that offline, and for online runs the normal pipeline + allowlist covers the case we care about.
- **Not** snapshotting all ~5000 corpus recipes. Allowlist-driven from the start.
- **Not** censoring image URLs. Deferred.
- **Not** changing how `data/pipeline-runs/<timestamp>/…` works.

## File layout after this change

```
data/
  pipeline-snapshot-urls.json     # allowlist, committed
  pipeline-snapshots/             # committed
    smittenkitchen-com_2014_03_sizzling-chicken-fajitas.json
    …
  pipeline-runs/                  # still gitignored
  pipeline-cache/                 # still gitignored
  test-urls.json                  # now includes the fajitas URL

ramekin-core/src/
  final_recipe.rs                 # new: FinalRecipe + build_final_recipe
  lib.rs                          # modified: re-export final_recipe module

cli/src/pipeline/
  snapshots.rs                    # new: write_snapshots
  mod.rs                          # modified: re-export snapshots module

cli/src/
  pipeline_orchestrator.rs        # modified: call write_snapshots after run completes
```

## Testing

- Unit tests in `ramekin-core` for `build_final_recipe` covering: all inputs present; parse_ingredients absent (line-split fallback); suggested but no applied tags; neither suggested nor applied tags.
- End-to-end: run `make pipeline SITE=smittenkitchen.com` with the fajitas URL in the allowlist; assert `data/pipeline-snapshots/smittenkitchen-com_2014_03_sizzling-chicken-fajitas.json` exists and has the expected top-level keys (`title`, `ingredients`, `suggested_tags`, etc.).
- Error path: add a bogus URL to the allowlist that isn't in `test-urls.json`; rerun pipeline; assert the run fails with a clear error pointing at the missing URL.
