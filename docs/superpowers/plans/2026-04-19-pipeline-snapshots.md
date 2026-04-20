# Pipeline Snapshots Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** After each `make pipeline` run, write consolidated end-of-pipeline recipe JSON for an allowlisted set of URLs to a committed `data/pipeline-snapshots/` directory, so algorithmic changes to extraction, ingredient parsing, and auto-tagging are visible in git diffs.

**Architecture:** A new `ramekin_core::final_recipe` module exposes a `FinalRecipe` struct and `build_final_recipe(...)` helper that takes the outputs of the relevant pipeline steps. A new `cli/src/pipeline/snapshots.rs` reads those step outputs from the run's output directory for each URL in `data/pipeline-snapshot-urls.json`, calls the helper, and writes `data/pipeline-snapshots/<slug>.json`. The call is wired into `pipeline_orchestrator.rs` at run completion. Missing allowlisted URLs are a hard error.

**Tech Stack:** Rust (workspace crates `ramekin-core`, `ramekin-cli`), serde_json, anyhow, standard library filesystem APIs. Tests run via `cd ramekin-core && cargo test` / `cd cli && cargo test`. Lint via `make lint`. Pipeline run via `make pipeline`.

**Spec:** `docs/superpowers/specs/2026-04-19-pipeline-snapshots-design.md`

---

## Conventions used in this plan

- **All builds and tests go through the cargo on the workspace crate**, e.g. `cd ramekin-core && cargo test <test_name>` — do not use `cargo build` at the workspace root, per `AGENTS.md` / Makefile conventions.
- **Use `tracing::info!` / `tracing::warn!`**, not `println!`/`eprintln!`, for status output inside library code. `println!` is acceptable inside the CLI's summary section where it already prints to stdout.
- **Fail fast.** Snapshot write errors abort the pipeline run. No graceful degradation.
- **No raw SQL** — not applicable here (no DB work in this plan).
- **Commit after each logically complete task.** Commit messages use imperative mood ("add X", "fix Y").
- **Do not modify generated code.** Not applicable here.

---

## Task 1: Add `FinalRecipe` type with the raw-recipe fields only

**Goal:** Create the type and a helper that populates only the fields sourced from `RawRecipe`, with no ingredient or tag handling yet. Get the module wired into the crate first, then layer in complexity.

**Files:**
- Create: `ramekin-core/src/final_recipe.rs`
- Modify: `ramekin-core/src/lib.rs`
- Test: in-module `#[cfg(test)]` block in `ramekin-core/src/final_recipe.rs`

- [ ] **Step 1: Write the failing test**

Create `ramekin-core/src/final_recipe.rs` with:

```rust
//! Consolidated end-of-pipeline recipe view.
//!
//! Assembles the final state of a scraped recipe by combining the outputs of
//! `extract_recipe`, `parse_ingredients`, `enrich_auto_tag`, and `apply_auto_tags`.
//! Used by the CLI snapshot writer and (eventually) the server scrape-status view.

use serde::{Deserialize, Serialize};

use crate::ingredient_parser::ParsedIngredient;
use crate::types::RawRecipe;

/// Consolidated end-of-pipeline recipe state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FinalRecipe {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servings: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prep_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cook_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_time: Option<String>,
    pub instructions: String,
    pub image_urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    pub ingredients: Vec<ParsedIngredient>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_tags: Option<Vec<String>>,
}

/// Build a `FinalRecipe` from the outputs of the relevant pipeline steps.
pub fn build_final_recipe(
    raw_recipe: &RawRecipe,
    parsed_ingredients: Option<&[ParsedIngredient]>,
    suggested_tags: Option<&[String]>,
    applied_tags: Option<&[String]>,
) -> FinalRecipe {
    let _ = (parsed_ingredients, suggested_tags, applied_tags);
    FinalRecipe {
        title: raw_recipe.title.clone(),
        description: raw_recipe.description.clone(),
        servings: raw_recipe.servings.clone(),
        prep_time: raw_recipe.prep_time.clone(),
        cook_time: raw_recipe.cook_time.clone(),
        total_time: raw_recipe.total_time.clone(),
        instructions: raw_recipe.instructions.clone(),
        image_urls: raw_recipe.image_urls.clone(),
        source_url: raw_recipe.source_url.clone(),
        source_name: raw_recipe.source_name.clone(),
        ingredients: Vec::new(),
        applied_tags: None,
        suggested_tags: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_recipe_fixture() -> RawRecipe {
        RawRecipe {
            title: "Test Recipe".to_string(),
            description: Some("A test".to_string()),
            ingredients: "1 cup flour\n2 eggs".to_string(),
            instructions: "Mix and bake.".to_string(),
            image_urls: vec!["https://example.com/img.jpg".to_string()],
            source_url: Some("https://example.com/recipe".to_string()),
            source_name: Some("example.com".to_string()),
            servings: Some("4".to_string()),
            prep_time: Some("10 minutes".to_string()),
            cook_time: Some("20 minutes".to_string()),
            total_time: Some("30 minutes".to_string()),
            rating: None,
            difficulty: None,
            nutritional_info: None,
            notes: None,
            categories: None,
            footnotes: None,
        }
    }

    #[test]
    fn copies_raw_recipe_fields() {
        let raw = raw_recipe_fixture();
        let fr = build_final_recipe(&raw, None, None, None);
        assert_eq!(fr.title, "Test Recipe");
        assert_eq!(fr.description.as_deref(), Some("A test"));
        assert_eq!(fr.instructions, "Mix and bake.");
        assert_eq!(fr.image_urls, vec!["https://example.com/img.jpg".to_string()]);
        assert_eq!(fr.source_name.as_deref(), Some("example.com"));
        assert_eq!(fr.servings.as_deref(), Some("4"));
        assert_eq!(fr.total_time.as_deref(), Some("30 minutes"));
    }
}
```

Add to `ramekin-core/src/lib.rs` (find the existing list of `pub mod` declarations — alphabetical — and insert):

```rust
pub mod final_recipe;
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cd ramekin-core && cargo test final_recipe::tests::copies_raw_recipe_fields`

Expected: PASS (compiles and the test passes).

- [ ] **Step 3: Commit**

```bash
git add ramekin-core/src/final_recipe.rs ramekin-core/src/lib.rs
git commit -m "Add FinalRecipe skeleton with raw-recipe fields"
```

---

## Task 2: Add ingredient handling with line-split fallback

**Goal:** Populate `FinalRecipe.ingredients` from parsed ingredients when available, otherwise fall back to line-splitting the raw ingredients string. This fallback logic is the same one currently in `server/src/scraping/steps.rs:334-353`.

**Files:**
- Modify: `ramekin-core/src/final_recipe.rs`
- Test: in-module `#[cfg(test)]`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `ramekin-core/src/final_recipe.rs`:

```rust
    #[test]
    fn uses_parsed_ingredients_when_present() {
        let raw = raw_recipe_fixture();
        let parsed = vec![ParsedIngredient {
            item: "flour".to_string(),
            measurements: vec![crate::ingredient_parser::Measurement {
                amount: Some("1".to_string()),
                unit: Some("cup".to_string()),
            }],
            note: None,
            raw: Some("1 cup flour".to_string()),
            section: None,
        }];
        let fr = build_final_recipe(&raw, Some(&parsed), None, None);
        assert_eq!(fr.ingredients.len(), 1);
        assert_eq!(fr.ingredients[0].item, "flour");
    }

    #[test]
    fn falls_back_to_line_split_when_parsed_absent() {
        let raw = raw_recipe_fixture();
        let fr = build_final_recipe(&raw, None, None, None);
        assert_eq!(fr.ingredients.len(), 2);
        assert_eq!(fr.ingredients[0].item, "1 cup flour");
        assert_eq!(fr.ingredients[0].measurements, Vec::new());
        assert_eq!(fr.ingredients[1].item, "2 eggs");
    }

    #[test]
    fn falls_back_to_line_split_when_parsed_empty() {
        let raw = raw_recipe_fixture();
        let empty: Vec<ParsedIngredient> = Vec::new();
        let fr = build_final_recipe(&raw, Some(&empty), None, None);
        assert_eq!(fr.ingredients.len(), 2);
    }

    #[test]
    fn line_split_skips_blank_lines() {
        let mut raw = raw_recipe_fixture();
        raw.ingredients = "1 cup flour\n\n   \n2 eggs".to_string();
        let fr = build_final_recipe(&raw, None, None, None);
        assert_eq!(fr.ingredients.len(), 2);
        assert_eq!(fr.ingredients[0].item, "1 cup flour");
        assert_eq!(fr.ingredients[1].item, "2 eggs");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd ramekin-core && cargo test final_recipe::tests`

Expected: FAIL — new tests fail (the function currently always returns `ingredients: Vec::new()`).

- [ ] **Step 3: Implement the fallback**

Replace the body of `build_final_recipe` in `ramekin-core/src/final_recipe.rs`. Change:

```rust
pub fn build_final_recipe(
    raw_recipe: &RawRecipe,
    parsed_ingredients: Option<&[ParsedIngredient]>,
    suggested_tags: Option<&[String]>,
    applied_tags: Option<&[String]>,
) -> FinalRecipe {
    let _ = (parsed_ingredients, suggested_tags, applied_tags);
    FinalRecipe {
        title: raw_recipe.title.clone(),
        description: raw_recipe.description.clone(),
        servings: raw_recipe.servings.clone(),
        prep_time: raw_recipe.prep_time.clone(),
        cook_time: raw_recipe.cook_time.clone(),
        total_time: raw_recipe.total_time.clone(),
        instructions: raw_recipe.instructions.clone(),
        image_urls: raw_recipe.image_urls.clone(),
        source_url: raw_recipe.source_url.clone(),
        source_name: raw_recipe.source_name.clone(),
        ingredients: Vec::new(),
        applied_tags: None,
        suggested_tags: None,
    }
}
```

to:

```rust
pub fn build_final_recipe(
    raw_recipe: &RawRecipe,
    parsed_ingredients: Option<&[ParsedIngredient]>,
    suggested_tags: Option<&[String]>,
    applied_tags: Option<&[String]>,
) -> FinalRecipe {
    let _ = (suggested_tags, applied_tags);

    let ingredients = match parsed_ingredients {
        Some(parsed) if !parsed.is_empty() => parsed.to_vec(),
        _ => raw_recipe
            .ingredients
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| ParsedIngredient {
                item: line.trim().to_string(),
                measurements: Vec::new(),
                note: None,
                raw: None,
                section: None,
            })
            .collect(),
    };

    FinalRecipe {
        title: raw_recipe.title.clone(),
        description: raw_recipe.description.clone(),
        servings: raw_recipe.servings.clone(),
        prep_time: raw_recipe.prep_time.clone(),
        cook_time: raw_recipe.cook_time.clone(),
        total_time: raw_recipe.total_time.clone(),
        instructions: raw_recipe.instructions.clone(),
        image_urls: raw_recipe.image_urls.clone(),
        source_url: raw_recipe.source_url.clone(),
        source_name: raw_recipe.source_name.clone(),
        ingredients,
        applied_tags: None,
        suggested_tags: None,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd ramekin-core && cargo test final_recipe::tests`

Expected: PASS — all five tests in the module pass.

- [ ] **Step 5: Commit**

```bash
git add ramekin-core/src/final_recipe.rs
git commit -m "Wire parsed ingredients + line-split fallback into FinalRecipe"
```

---

## Task 3: Add tag handling

**Goal:** Populate `applied_tags` and `suggested_tags` when provided; de-duplicate `suggested_tags` against `applied_tags` (no — keep them both verbatim, let the caller decide). Actually: carry both through verbatim; keep them untouched so the snapshot shows what each step produced.

**Files:**
- Modify: `ramekin-core/src/final_recipe.rs`
- Test: in-module `#[cfg(test)]`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module:

```rust
    #[test]
    fn passes_suggested_tags_through() {
        let raw = raw_recipe_fixture();
        let suggested = vec!["dinner".to_string(), "mexican".to_string()];
        let fr = build_final_recipe(&raw, None, Some(&suggested), None);
        assert_eq!(fr.suggested_tags.as_deref(), Some(&suggested[..]));
        assert!(fr.applied_tags.is_none());
    }

    #[test]
    fn passes_applied_tags_through() {
        let raw = raw_recipe_fixture();
        let applied = vec!["vegetarian".to_string()];
        let fr = build_final_recipe(&raw, None, None, Some(&applied));
        assert_eq!(fr.applied_tags.as_deref(), Some(&applied[..]));
        assert!(fr.suggested_tags.is_none());
    }

    #[test]
    fn passes_both_tag_fields_through() {
        let raw = raw_recipe_fixture();
        let suggested = vec!["dinner".to_string()];
        let applied = vec!["dinner".to_string(), "mexican".to_string()];
        let fr = build_final_recipe(&raw, None, Some(&suggested), Some(&applied));
        assert_eq!(fr.suggested_tags.as_deref(), Some(&suggested[..]));
        assert_eq!(fr.applied_tags.as_deref(), Some(&applied[..]));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd ramekin-core && cargo test final_recipe::tests`

Expected: FAIL — new tag tests fail because the function currently always sets `applied_tags: None, suggested_tags: None`.

- [ ] **Step 3: Implement tag passthrough**

In `ramekin-core/src/final_recipe.rs`, replace the function body's final return expression to plumb tags through. Change:

```rust
    let _ = (suggested_tags, applied_tags);
```

to: *(remove that line entirely)*

And change:

```rust
        applied_tags: None,
        suggested_tags: None,
```

to:

```rust
        applied_tags: applied_tags.map(<[String]>::to_vec),
        suggested_tags: suggested_tags.map(<[String]>::to_vec),
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd ramekin-core && cargo test final_recipe::tests`

Expected: PASS — all eight tests in the module pass.

- [ ] **Step 5: Commit**

```bash
git add ramekin-core/src/final_recipe.rs
git commit -m "Plumb applied and suggested tags through FinalRecipe"
```

---

## Task 4: Add serialization stability test

**Goal:** Verify the serialized JSON has stable, predictable output — absent optional fields shouldn't appear as `null`, and the type round-trips through serde cleanly. This guards against future diff noise.

**Files:**
- Modify: `ramekin-core/src/final_recipe.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
    #[test]
    fn serializes_without_null_fields_for_missing_optionals() {
        let raw = raw_recipe_fixture();
        let fr = build_final_recipe(&raw, None, None, None);
        let json = serde_json::to_string(&fr).unwrap();
        assert!(!json.contains("\"applied_tags\""), "unexpected applied_tags in {json}");
        assert!(!json.contains("\"suggested_tags\""), "unexpected suggested_tags in {json}");
    }

    #[test]
    fn round_trips_through_serde() {
        let raw = raw_recipe_fixture();
        let suggested = vec!["dinner".to_string()];
        let applied = vec!["vegetarian".to_string()];
        let fr = build_final_recipe(&raw, None, Some(&suggested), Some(&applied));
        let json = serde_json::to_string_pretty(&fr).unwrap();
        let decoded: FinalRecipe = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, fr);
    }
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cd ramekin-core && cargo test final_recipe::tests`

Expected: PASS — the serialization tests pass because `#[serde(skip_serializing_if = "Option::is_none")]` is already in place.

- [ ] **Step 3: Commit**

```bash
git add ramekin-core/src/final_recipe.rs
git commit -m "Add serialization stability tests for FinalRecipe"
```

---

## Task 5: Add the snapshot allowlist file and seed URL

**Goal:** Create the allowlist file and add the smittenkitchen URL to the test-urls list. This is a pure data change with no code yet — but committing it separately makes the plan reviewable.

**Files:**
- Create: `data/pipeline-snapshot-urls.json`
- Modify: `data/test-urls.json` (append URL only if not already present)

- [ ] **Step 1: Check whether the URL is already in `data/test-urls.json`**

Run: `grep -c 'smittenkitchen.com/2014/03/sizzling-chicken-fajitas/' data/test-urls.json`

If output is `0`, add it (see Step 2). If it's `1` or more, skip Step 2.

- [ ] **Step 2: Add URL to `data/test-urls.json` if missing**

`data/test-urls.json` is a JSON array of URLs (verify by running `jq 'type' data/test-urls.json` → should print `"array"`). Append the URL:

Run:

```bash
jq '. + ["https://smittenkitchen.com/2014/03/sizzling-chicken-fajitas/"]' data/test-urls.json > data/test-urls.json.tmp && mv data/test-urls.json.tmp data/test-urls.json
```

If `data/test-urls.json` is not a JSON array of URL strings (inspect the top of the file first with `head -5 data/test-urls.json` before running the command above), adapt the jq expression to match the actual schema. Leave a note in the commit message describing what you did.

- [ ] **Step 3: Create `data/pipeline-snapshot-urls.json`**

Content (exact text):

```json
[
  "https://smittenkitchen.com/2014/03/sizzling-chicken-fajitas/"
]
```

- [ ] **Step 4: Verify both files are valid JSON**

Run: `jq . data/pipeline-snapshot-urls.json > /dev/null && jq . data/test-urls.json > /dev/null`

Expected: no output, exit code 0.

- [ ] **Step 5: Commit**

```bash
git add data/pipeline-snapshot-urls.json data/test-urls.json
git commit -m "Seed pipeline snapshot allowlist with smittenkitchen fajitas URL"
```

---

## Task 6: Stub the snapshot writer module

**Goal:** Create `cli/src/pipeline/snapshots.rs` with the module skeleton and public API. Wire it into `cli/src/pipeline/mod.rs` so it's reachable. No logic yet — this is scaffolding to keep subsequent tasks focused.

**Files:**
- Create: `cli/src/pipeline/snapshots.rs`
- Modify: `cli/src/pipeline/mod.rs`

- [ ] **Step 1: Inspect the existing `cli/src/pipeline/mod.rs` to find the module declarations**

Run: `head -20 cli/src/pipeline/mod.rs`

Note the existing `mod output_store;`, `mod runners;`, etc. lines — these live near the top.

- [ ] **Step 2: Create `cli/src/pipeline/snapshots.rs`**

Content (exact text):

```rust
//! Write end-of-pipeline recipe snapshots for an allowlisted set of URLs.
//!
//! After a pipeline run completes, this module reads the relevant per-step
//! outputs from `run_dir/urls/<slug>/` for each allowlisted URL, assembles a
//! `FinalRecipe` via `ramekin_core::final_recipe::build_final_recipe`, and
//! writes the JSON to `snapshots_dir/<slug>.json`. If an allowlisted URL
//! isn't present in the run directory, the function returns an error so the
//! pipeline run fails fast.

use std::path::Path;

use anyhow::Result;

/// Write snapshots for every URL in `allowlist_path` by reading step outputs
/// under `run_dir` and writing JSON files under `snapshots_dir`.
pub fn write_snapshots(
    run_dir: &Path,
    allowlist_path: &Path,
    snapshots_dir: &Path,
) -> Result<()> {
    let _ = (run_dir, allowlist_path, snapshots_dir);
    Ok(())
}
```

- [ ] **Step 3: Add the module declaration in `cli/src/pipeline/mod.rs`**

Insert (alphabetically in the existing `mod` block near the top of the file):

```rust
pub mod snapshots;
```

- [ ] **Step 4: Verify the crate builds**

Run: `cd cli && cargo build`

Expected: builds cleanly.

- [ ] **Step 5: Commit**

```bash
git add cli/src/pipeline/snapshots.rs cli/src/pipeline/mod.rs
git commit -m "Stub pipeline snapshot writer module"
```

---

## Task 7: Allowlist parsing, with tests

**Goal:** Add a helper that reads the allowlist JSON and returns `Vec<String>`. Tested before wiring into the writer.

**Files:**
- Modify: `cli/src/pipeline/snapshots.rs`

- [ ] **Step 1: Write the failing test**

Append to `cli/src/pipeline/snapshots.rs`:

```rust
fn read_allowlist(path: &Path) -> Result<Vec<String>> {
    use anyhow::Context;
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read allowlist: {}", path.display()))?;
    let urls: Vec<String> = serde_json::from_str(&text)
        .with_context(|| format!("Failed to parse allowlist JSON: {}", path.display()))?;
    Ok(urls)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn reads_valid_allowlist() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("allowlist.json");
        std::fs::write(&path, r#"["https://a.example/", "https://b.example/"]"#).unwrap();
        let urls = read_allowlist(&path).unwrap();
        assert_eq!(
            urls,
            vec!["https://a.example/".to_string(), "https://b.example/".to_string()],
        );
    }

    #[test]
    fn reads_empty_allowlist() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("allowlist.json");
        std::fs::write(&path, "[]").unwrap();
        let urls = read_allowlist(&path).unwrap();
        assert!(urls.is_empty());
    }

    #[test]
    fn errors_on_missing_file() {
        let err = read_allowlist(Path::new("/nonexistent/allowlist.json")).unwrap_err();
        assert!(err.to_string().contains("Failed to read allowlist"));
    }

    #[test]
    fn errors_on_invalid_json() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("allowlist.json");
        std::fs::write(&path, "not json").unwrap();
        let err = read_allowlist(&path).unwrap_err();
        assert!(err.to_string().contains("Failed to parse allowlist JSON"));
    }
}
```

- [ ] **Step 2: Check whether `tempfile` is already a dev-dependency**

Run: `grep -A20 '\[dev-dependencies\]' cli/Cargo.toml | head -30`

If `tempfile` is listed, proceed to Step 4. Otherwise, Step 3.

- [ ] **Step 3: Add `tempfile` as a dev-dependency if missing**

Inspect `cli/Cargo.toml`. If there's no `[dev-dependencies]` section, add one at the bottom of the file:

```toml
[dev-dependencies]
tempfile = "3"
```

If the section exists, add `tempfile = "3"` to it (alphabetically).

Run: `cd cli && cargo build --tests` to fetch the dependency.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd cli && cargo test -p ramekin-cli pipeline::snapshots::tests`

Expected: PASS — all four tests pass.

- [ ] **Step 5: Commit**

```bash
git add cli/src/pipeline/snapshots.rs cli/Cargo.toml cli/Cargo.lock
git commit -m "Add allowlist parsing with tests"
```

---

## Task 8: Step output reader helper

**Goal:** Add a helper that, given `run_dir`, a URL slug, and a step name, reads `run_dir/urls/<slug>/<step>/output.json` and returns `Option<serde_json::Value>`. Returns `None` if the file doesn't exist; returns `Err` if the file is present but unreadable or not JSON. This isolates disk I/O concerns from the higher-level logic.

**Files:**
- Modify: `cli/src/pipeline/snapshots.rs`

- [ ] **Step 1: Write the failing test**

Append to `cli/src/pipeline/snapshots.rs` (inside the existing `mod tests`):

```rust
    #[test]
    fn reads_step_output_when_present() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path();
        let step_dir = run_dir.join("urls").join("example-com_recipe").join("extract_recipe");
        std::fs::create_dir_all(&step_dir).unwrap();
        std::fs::write(step_dir.join("output.json"), r#"{"foo":"bar"}"#).unwrap();

        let out = read_step_output(run_dir, "example-com_recipe", "extract_recipe").unwrap();
        assert_eq!(out, Some(serde_json::json!({"foo": "bar"})));
    }

    #[test]
    fn returns_none_when_step_output_missing() {
        let dir = TempDir::new().unwrap();
        let out = read_step_output(dir.path(), "example-com_recipe", "extract_recipe").unwrap();
        assert!(out.is_none());
    }

    #[test]
    fn errors_when_step_output_is_malformed_json() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path();
        let step_dir = run_dir.join("urls").join("example-com_recipe").join("extract_recipe");
        std::fs::create_dir_all(&step_dir).unwrap();
        std::fs::write(step_dir.join("output.json"), "not json").unwrap();

        let err = read_step_output(run_dir, "example-com_recipe", "extract_recipe").unwrap_err();
        assert!(err.to_string().contains("Failed to parse"));
    }
```

- [ ] **Step 2: Run tests to verify they fail to compile**

Run: `cd cli && cargo test -p ramekin-cli pipeline::snapshots::tests`

Expected: FAIL — compiler error because `read_step_output` doesn't exist.

- [ ] **Step 3: Implement `read_step_output`**

Add this function to `cli/src/pipeline/snapshots.rs` (above the `#[cfg(test)]` block):

```rust
fn read_step_output(
    run_dir: &Path,
    url_slug: &str,
    step_name: &str,
) -> Result<Option<serde_json::Value>> {
    use anyhow::Context;
    let path = run_dir
        .join("urls")
        .join(url_slug)
        .join(step_name)
        .join("output.json");
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read step output: {}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("Failed to parse step output JSON: {}", path.display()))?;
    Ok(Some(value))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd cli && cargo test -p ramekin-cli pipeline::snapshots::tests`

Expected: PASS — all seven tests in the module pass.

- [ ] **Step 5: Commit**

```bash
git add cli/src/pipeline/snapshots.rs
git commit -m "Add step output reader helper for snapshot writer"
```

---

## Task 9: Assemble `FinalRecipe` from step outputs

**Goal:** Add a helper that takes `run_dir` + `url_slug`, reads the needed step outputs, and returns `FinalRecipe`. Errors if `extract_recipe` is missing (URL wasn't processed or pipeline didn't even get to extract); tolerates missing `parse_ingredients`, `enrich_auto_tag`, `apply_auto_tags`.

**Files:**
- Modify: `cli/src/pipeline/snapshots.rs`

- [ ] **Step 1: Write the failing tests**

Append inside the existing `mod tests` in `cli/src/pipeline/snapshots.rs`:

```rust
    fn write_step_output(run_dir: &Path, slug: &str, step: &str, body: &str) {
        let dir = run_dir.join("urls").join(slug).join(step);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("output.json"), body).unwrap();
    }

    fn extract_output_body() -> &'static str {
        r#"{
          "raw_recipe": {
            "title": "Test",
            "ingredients": "1 cup flour\n2 eggs",
            "instructions": "Mix.",
            "image_urls": []
          },
          "method_used": "json_ld"
        }"#
    }

    #[test]
    fn assembles_with_only_extract_recipe() {
        let dir = TempDir::new().unwrap();
        write_step_output(dir.path(), "example-com_r", "extract_recipe", extract_output_body());

        let fr = assemble_snapshot(dir.path(), "example-com_r").unwrap();
        assert_eq!(fr.title, "Test");
        assert_eq!(fr.ingredients.len(), 2); // line-split fallback
        assert!(fr.applied_tags.is_none());
        assert!(fr.suggested_tags.is_none());
    }

    #[test]
    fn assembles_with_parse_ingredients_and_tags() {
        let dir = TempDir::new().unwrap();
        let slug = "example-com_r";
        write_step_output(dir.path(), slug, "extract_recipe", extract_output_body());
        write_step_output(
            dir.path(),
            slug,
            "parse_ingredients",
            r#"{
              "ingredients": [
                {
                  "item": "flour",
                  "measurements": [{"amount": "1", "unit": "cup"}],
                  "note": null,
                  "raw": "1 cup flour",
                  "section": null
                }
              ]
            }"#,
        );
        write_step_output(
            dir.path(),
            slug,
            "enrich_auto_tag",
            r#"{"suggested_tags": ["dinner", "breakfast"]}"#,
        );
        write_step_output(
            dir.path(),
            slug,
            "apply_auto_tags",
            r#"{"tags_applied": ["dinner"]}"#,
        );

        let fr = assemble_snapshot(dir.path(), slug).unwrap();
        assert_eq!(fr.ingredients.len(), 1);
        assert_eq!(fr.ingredients[0].item, "flour");
        assert_eq!(fr.suggested_tags.as_deref(), Some(&["dinner".to_string(), "breakfast".to_string()][..]));
        assert_eq!(fr.applied_tags.as_deref(), Some(&["dinner".to_string()][..]));
    }

    #[test]
    fn errors_when_extract_recipe_output_missing() {
        let dir = TempDir::new().unwrap();
        let err = assemble_snapshot(dir.path(), "missing-slug").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("missing-slug"), "error should name the slug: {msg}");
        assert!(msg.contains("extract_recipe"), "error should mention extract_recipe: {msg}");
    }
```

- [ ] **Step 2: Run tests to verify they fail to compile**

Run: `cd cli && cargo test -p ramekin-cli pipeline::snapshots::tests`

Expected: FAIL — `assemble_snapshot` doesn't exist.

- [ ] **Step 3: Implement `assemble_snapshot`**

Add to `cli/src/pipeline/snapshots.rs` (above the `#[cfg(test)]` block):

```rust
use ramekin_core::final_recipe::{build_final_recipe, FinalRecipe};
use ramekin_core::ingredient_parser::ParsedIngredient;
use ramekin_core::types::RawRecipe;

fn assemble_snapshot(run_dir: &Path, url_slug: &str) -> Result<FinalRecipe> {
    use anyhow::{anyhow, Context};

    let extract = read_step_output(run_dir, url_slug, "extract_recipe")?
        .ok_or_else(|| {
            anyhow!(
                "No extract_recipe output for URL slug {url_slug} in {}. \
                 Either the URL wasn't in the pipeline run or the pipeline didn't reach extract_recipe.",
                run_dir.display()
            )
        })?;

    let raw_recipe: RawRecipe = serde_json::from_value(
        extract
            .get("raw_recipe")
            .cloned()
            .ok_or_else(|| anyhow!("extract_recipe output missing raw_recipe field for {url_slug}"))?,
    )
    .context("Failed to deserialize raw_recipe from extract_recipe output")?;

    let parsed_ingredients: Option<Vec<ParsedIngredient>> = read_step_output(
        run_dir,
        url_slug,
        "parse_ingredients",
    )?
    .and_then(|v| v.get("ingredients").cloned())
    .map(serde_json::from_value)
    .transpose()
    .context("Failed to deserialize parse_ingredients output")?;

    let suggested_tags: Option<Vec<String>> =
        read_step_output(run_dir, url_slug, "enrich_auto_tag")?
            .and_then(|v| v.get("suggested_tags").cloned())
            .map(serde_json::from_value)
            .transpose()
            .context("Failed to deserialize enrich_auto_tag suggested_tags")?;

    let applied_tags: Option<Vec<String>> =
        read_step_output(run_dir, url_slug, "apply_auto_tags")?
            .and_then(|v| v.get("tags_applied").cloned())
            .map(serde_json::from_value)
            .transpose()
            .context("Failed to deserialize apply_auto_tags tags_applied")?;

    Ok(build_final_recipe(
        &raw_recipe,
        parsed_ingredients.as_deref(),
        suggested_tags.as_deref(),
        applied_tags.as_deref(),
    ))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd cli && cargo test -p ramekin-cli pipeline::snapshots::tests`

Expected: PASS — all ten tests in the module pass.

- [ ] **Step 5: Commit**

```bash
git add cli/src/pipeline/snapshots.rs
git commit -m "Assemble FinalRecipe from step outputs in snapshot writer"
```

---

## Task 10: Implement `write_snapshots` end-to-end

**Goal:** Flesh out the top-level `write_snapshots` function. It reads the allowlist, iterates URLs, computes slugs via `ramekin_core::http::slugify_url`, calls `assemble_snapshot`, and writes to `snapshots_dir/<slug>.json` (creating the directory if needed). Any URL that fails assembly errors the whole call.

**Files:**
- Modify: `cli/src/pipeline/snapshots.rs`

- [ ] **Step 1: Write the failing test**

Append inside the existing `mod tests`:

```rust
    use ramekin_core::http::slugify_url;

    #[test]
    fn writes_snapshot_files_for_allowlisted_urls() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path().join("run");
        let snapshots_dir = dir.path().join("snapshots");
        let url = "https://example.com/r";
        let slug = slugify_url(url);
        write_step_output(&run_dir, &slug, "extract_recipe", extract_output_body());

        let allowlist = dir.path().join("allowlist.json");
        std::fs::write(&allowlist, format!("[{:?}]", url)).unwrap();

        write_snapshots(&run_dir, &allowlist, &snapshots_dir).unwrap();

        let snapshot_path = snapshots_dir.join(format!("{slug}.json"));
        assert!(snapshot_path.exists(), "snapshot not written at {}", snapshot_path.display());
        let content = std::fs::read_to_string(&snapshot_path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(value.get("title").and_then(|v| v.as_str()), Some("Test"));
    }

    #[test]
    fn errors_when_allowlisted_url_missing_from_run() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path().join("run");
        std::fs::create_dir_all(&run_dir).unwrap();
        let snapshots_dir = dir.path().join("snapshots");
        let allowlist = dir.path().join("allowlist.json");
        std::fs::write(&allowlist, r#"["https://missing.example/"]"#).unwrap();

        let err = write_snapshots(&run_dir, &allowlist, &snapshots_dir).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("https://missing.example/"), "error should name the URL: {msg}");
    }

    #[test]
    fn snapshot_output_is_pretty_printed() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path().join("run");
        let snapshots_dir = dir.path().join("snapshots");
        let url = "https://example.com/r";
        let slug = slugify_url(url);
        write_step_output(&run_dir, &slug, "extract_recipe", extract_output_body());

        let allowlist = dir.path().join("allowlist.json");
        std::fs::write(&allowlist, format!("[{:?}]", url)).unwrap();
        write_snapshots(&run_dir, &allowlist, &snapshots_dir).unwrap();

        let snapshot = std::fs::read_to_string(snapshots_dir.join(format!("{slug}.json"))).unwrap();
        assert!(snapshot.contains('\n'), "expected pretty-printed multi-line JSON");
        assert!(snapshot.ends_with('\n'), "expected trailing newline");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd cli && cargo test -p ramekin-cli pipeline::snapshots::tests`

Expected: FAIL — `write_snapshots` still has the stub body that returns `Ok(())` without doing anything, so the files-exist assertions fail.

- [ ] **Step 3: Implement `write_snapshots`**

Replace the stub `write_snapshots` in `cli/src/pipeline/snapshots.rs`:

```rust
pub fn write_snapshots(
    run_dir: &Path,
    allowlist_path: &Path,
    snapshots_dir: &Path,
) -> Result<()> {
    use anyhow::Context;
    use ramekin_core::http::slugify_url;

    let urls = read_allowlist(allowlist_path)?;
    if urls.is_empty() {
        return Ok(());
    }

    std::fs::create_dir_all(snapshots_dir)
        .with_context(|| format!("Failed to create {}", snapshots_dir.display()))?;

    for url in &urls {
        let slug = slugify_url(url);
        let final_recipe = assemble_snapshot(run_dir, &slug).with_context(|| {
            format!("Failed to assemble snapshot for allowlisted URL {url}")
        })?;

        let mut json = serde_json::to_string_pretty(&final_recipe)
            .context("Failed to serialize FinalRecipe")?;
        json.push('\n');

        let path = snapshots_dir.join(format!("{slug}.json"));
        std::fs::write(&path, json)
            .with_context(|| format!("Failed to write snapshot: {}", path.display()))?;

        tracing::info!("Wrote pipeline snapshot: {}", path.display());
    }

    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd cli && cargo test -p ramekin-cli pipeline::snapshots::tests`

Expected: PASS — all thirteen tests in the module pass.

- [ ] **Step 5: Commit**

```bash
git add cli/src/pipeline/snapshots.rs
git commit -m "Implement write_snapshots end-to-end"
```

---

## Task 11: Wire `write_snapshots` into the pipeline orchestrator

**Goal:** Call `write_snapshots` from `run_pipeline` in `cli/src/pipeline_orchestrator.rs` after the main run completes but before the final manifest write. The call site uses fixed paths `data/pipeline-snapshot-urls.json` and `data/pipeline-snapshots/`. If the allowlist file doesn't exist, skip with a warning (so people who haven't set it up yet aren't broken); if it exists, failures abort the run.

**Files:**
- Modify: `cli/src/pipeline_orchestrator.rs`

- [ ] **Step 1: Find the call site**

Run: `grep -n "save_manifest(&run_dir, &final_manifest)" cli/src/pipeline_orchestrator.rs`

Expected: one hit around line 454 (the second `save_manifest` call, after `elapsed` is computed).

- [ ] **Step 2: Inspect the surrounding code**

Run: `sed -n '440,465p' cli/src/pipeline_orchestrator.rs`

Confirm the block looks like:

```rust
    let elapsed = start_time.elapsed();

    // Update manifest with completion
    let final_manifest = RunManifest {
        completed_at: Some(Utc::now().to_rfc3339()),
        status: RunStatus::Completed,
        ..manifest
    };
    save_manifest(&run_dir, &final_manifest)?;
```

- [ ] **Step 3: Insert the snapshot call**

Modify `cli/src/pipeline_orchestrator.rs`. Change:

```rust
    let elapsed = start_time.elapsed();

    // Update manifest with completion
    let final_manifest = RunManifest {
        completed_at: Some(Utc::now().to_rfc3339()),
        status: RunStatus::Completed,
        ..manifest
    };
    save_manifest(&run_dir, &final_manifest)?;
```

to:

```rust
    let elapsed = start_time.elapsed();

    // Write allowlisted per-URL snapshots before the final manifest so that a
    // snapshot failure propagates as a failed run.
    write_pipeline_snapshots(&run_dir)?;

    // Update manifest with completion
    let final_manifest = RunManifest {
        completed_at: Some(Utc::now().to_rfc3339()),
        status: RunStatus::Completed,
        ..manifest
    };
    save_manifest(&run_dir, &final_manifest)?;
```

- [ ] **Step 4: Add the helper function at the bottom of the file**

Append to `cli/src/pipeline_orchestrator.rs`:

```rust
fn write_pipeline_snapshots(run_dir: &Path) -> Result<()> {
    let allowlist = PathBuf::from("data/pipeline-snapshot-urls.json");
    let snapshots_dir = PathBuf::from("data/pipeline-snapshots");

    if !allowlist.exists() {
        tracing::warn!(
            "Snapshot allowlist {} not found; skipping snapshot write",
            allowlist.display()
        );
        return Ok(());
    }

    crate::pipeline::snapshots::write_snapshots(run_dir, &allowlist, &snapshots_dir)
}
```

If `PathBuf` / `Path` / `Result` aren't already imported in that file, add them. Run `grep -n 'use ' cli/src/pipeline_orchestrator.rs | head -20` to check, and add missing imports to the top.

- [ ] **Step 5: Verify the crate builds**

Run: `cd cli && cargo build`

Expected: builds cleanly.

- [ ] **Step 6: Verify unit tests still pass**

Run: `cd cli && cargo test -p ramekin-cli`

Expected: all tests pass, no regressions.

- [ ] **Step 7: Commit**

```bash
git add cli/src/pipeline_orchestrator.rs
git commit -m "Wire snapshot writer into pipeline orchestrator"
```

---

## Task 12: End-to-end run and commit the resulting snapshot

**Goal:** Actually run `make pipeline` filtered to the smittenkitchen site, confirm the snapshot file lands, and commit it. This also validates the whole pipeline tolerates the new step.

**Files:**
- Create (via pipeline output): `data/pipeline-snapshots/smittenkitchen-com_2014_03_sizzling-chicken-fajitas.json` (exact slug computed by `slugify_url`)
- May also update: other `data/` files that `make pipeline` regenerates (e.g. `extraction-report.md`, `tag-report.md`) — these are expected per AGENTS.md guidance.

- [ ] **Step 1: Run the pipeline filtered to smittenkitchen**

Run: `make pipeline SITE=smittenkitchen.com`

Expected: the run completes. Near the end, `tracing::info!` log lines like `Wrote pipeline snapshot: data/pipeline-snapshots/smittenkitchen-com_2014_03_sizzling-chicken-fajitas.json` should appear. The run should exit 0.

If the exact slug differs from what's shown above, use the actual value in the next step. Run `ls data/pipeline-snapshots/` to see what was written.

- [ ] **Step 2: Inspect the snapshot contents**

Run: `jq '.title, .ingredients | length, .suggested_tags' data/pipeline-snapshots/smittenkitchen-com_2014_03_sizzling-chicken-fajitas.json`

Expected: prints the recipe title, an ingredient count (> 0), and whatever tags enrich_auto_tag produced (possibly null if that step didn't run).

- [ ] **Step 3: Check the full diff**

Run: `git status` and `git diff data/`

Expected: the new snapshot file appears as untracked. Other `data/` files (`extraction-report.md`, `tag-report.md`, etc.) may have updates — inspect briefly to confirm the changes are consistent with only running the smittenkitchen slice.

- [ ] **Step 4: Commit the snapshot and any related data diffs**

Per AGENTS.md: commit the `data/` diffs that result from a pipeline change — they're the point.

```bash
git add data/pipeline-snapshots/ data/extraction-report.md data/tag-report.md data/ingredients.json data/unique-ingredients.txt
git status
```

If `git status` shows other `data/` files modified, add those too. Do not `git add -A`.

Commit:

```bash
git commit -m "Commit initial pipeline snapshot for smittenkitchen fajitas"
```

---

## Task 13: Error-path verification

**Goal:** Manually verify the hard-error behaviour when an allowlisted URL isn't in the current run's URL set. This is a one-off sanity check — no commit needed unless you find a bug.

**Files:** no permanent changes.

- [ ] **Step 1: Temporarily add a bogus URL to the allowlist**

Run:

```bash
cp data/pipeline-snapshot-urls.json /tmp/pipeline-snapshot-urls.json.bak
jq '. + ["https://nowhere.invalid/recipe/"]' data/pipeline-snapshot-urls.json > /tmp/new.json && mv /tmp/new.json data/pipeline-snapshot-urls.json
```

- [ ] **Step 2: Rerun the pipeline with the smittenkitchen filter**

Run: `make pipeline SITE=smittenkitchen.com`

Expected: the run exits non-zero. The error message should mention `https://nowhere.invalid/recipe/` and say something about the missing extract_recipe output or the URL not being in the pipeline run.

- [ ] **Step 3: Restore the allowlist**

Run: `cp /tmp/pipeline-snapshot-urls.json.bak data/pipeline-snapshot-urls.json && rm /tmp/pipeline-snapshot-urls.json.bak`

Run: `git diff data/pipeline-snapshot-urls.json`

Expected: no diff.

- [ ] **Step 4: Re-run pipeline to confirm clean run still works**

Run: `make pipeline SITE=smittenkitchen.com`

Expected: exits 0; no new changes to commit (the snapshot file is already up-to-date from Task 12).

---

## Task 14: Final lint + final commit

**Goal:** Run `make lint` across the repo, fix anything in the new files only (skip any pre-existing lint errors unrelated to this work — flag them to the user rather than fixing them silently).

**Files:** whatever the linter flags.

- [ ] **Step 1: Run the linter**

Run: `make lint`

- [ ] **Step 2: Triage results**

If failures are in `ramekin-core/src/final_recipe.rs`, `cli/src/pipeline/snapshots.rs`, `cli/src/pipeline_orchestrator.rs`, or `cli/src/pipeline/mod.rs`: fix them.

If failures are elsewhere (e.g. pre-existing TypeScript breakage in `ramekin-ui`), surface them to the user and do NOT attempt to fix them as part of this PR.

- [ ] **Step 3: Commit any lint fixes**

```bash
git add <files>
git commit -m "Fix lint for pipeline snapshot changes"
```

(Skip this step if there were no fixes required.)

---

## Self-Review Notes

Spec coverage:
- Shared `build_final_recipe` in `ramekin-core` — Tasks 1–4.
- Allowlist file at `data/pipeline-snapshot-urls.json` + URL in `data/test-urls.json` — Task 5.
- Snapshot writer module + wiring — Tasks 6–11.
- Image URL determinism: no code change required (passed through verbatim); verified via Task 12's inspection.
- "Fail fast" on missing allowlisted URL — Tasks 9, 10, 13.
- Non-goals (no new CLI subcommand, no changes to save_recipe in CLI/server) — respected.

Type consistency: `FinalRecipe`, `build_final_recipe`, `write_snapshots`, `assemble_snapshot`, `read_allowlist`, `read_step_output` names are used consistently across tasks.
