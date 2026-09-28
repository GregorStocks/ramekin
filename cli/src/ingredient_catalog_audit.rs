//! Ingredient catalog audit.
//!
//! Replays the ingredient-name matchers (nutrition, density, shopping category)
//! over recipe corpora and reports how much of each corpus they recognize, so
//! catalog changes show measurable before/after numbers.

use anyhow::{Context, Result};
use ramekin_core::final_recipe::FinalRecipe;
use ramekin_core::ingredient_categorizer::categorize;
use ramekin_core::ingredient_parser::{Measurement, ParsedIngredient};
use ramekin_core::nutrition;
use ramekin_core::types::ParseIngredientsOutput;
use ramekin_core::volume_to_weight::{find_density, is_volume_unit};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const FIXTURES_DIR: &str = "ramekin-core/tests/fixtures/ingredient_parsing";
const SNAPSHOTS_DIR: &str = "data/pipeline-snapshots";
const SHOPPING_CORPUS: &str = "data/shopping-list-categories.json";
const COMMITTED_REPORT: &str = "data/ingredient-catalog-audit.md";
const LOCAL_REPORT: &str = "logs/ingredient-catalog-audit-local.md";
const TOP_NAMES: usize = 30;

/// Nutrition failure reasons that mean the ingredient name itself was not
/// recognized (as opposed to a recognized food with an unusable quantity).
const NAME_FAILURES: [&str; 2] = ["No supported nutrition match", "Ambiguous ingredient"];

struct Recipe {
    servings: Option<String>,
    ingredients: Vec<ParsedIngredient>,
}

struct Corpus {
    name: String,
    recipes: Vec<Recipe>,
}

#[derive(Default)]
struct RecipeCorpusStats {
    recipes: usize,
    recipes_with_servings: usize,
    lines: usize,
    nutrition_name_recognized: usize,
    nutrition_computed: usize,
    recipes_fully_estimated: usize,
    recipes_with_per_serving: usize,
    nutrition_reasons: BTreeMap<String, usize>,
    nutrition_unrecognized: HashMap<String, usize>,
    volume_lines: usize,
    density_hits: usize,
    density_misses: HashMap<String, usize>,
    categorized: usize,
    uncategorized: HashMap<String, usize>,
}

#[derive(Default)]
struct ShoppingStats {
    items: usize,
    uses: u64,
    nutrition_name_recognized: usize,
    nutrition_name_recognized_uses: u64,
    categorized: usize,
    categorized_uses: u64,
    nutrition_unrecognized: HashMap<String, usize>,
    uncategorized: HashMap<String, usize>,
}

#[derive(Deserialize)]
struct FixtureFile {
    ingredients: Vec<FixtureLine>,
}

#[derive(Deserialize)]
struct FixtureLine {
    expected: Option<FixtureExpected>,
}

#[derive(Deserialize)]
struct FixtureExpected {
    item: String,
    measurements: Vec<Measurement>,
    note: Option<String>,
    section: Option<String>,
}

#[derive(Deserialize)]
struct ShoppingItem {
    item: String,
    count: u64,
}

/// The fields of a pipeline run's `manifest.json` that the audit checks.
#[derive(Deserialize)]
struct RunManifest {
    status: String,
}

#[derive(Deserialize)]
struct ProdRecipe {
    servings: Option<String>,
    ingredients: Vec<ParsedIngredient>,
}

fn normalize_name(item: &str) -> String {
    item.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// JSON files in `dir`, sorted by name so reports are deterministic.
fn sorted_json_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = fs::read_dir(dir)
        .with_context(|| format!("Failed to read {}", dir.display()))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    files.retain(|path| path.extension().is_some_and(|ext| ext == "json"));
    files.sort();
    Ok(files)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))
}

/// Recipe-shaped parser fixtures (pipeline and paprika); curated fixtures are
/// single-line edge cases, not recipes, so they are excluded.
fn load_fixture_corpus(root: &Path, subdir: &str, name: &str) -> Result<Corpus> {
    let mut recipes = Vec::new();
    for path in sorted_json_files(&root.join(FIXTURES_DIR).join(subdir))? {
        let file: FixtureFile = read_json(&path)?;
        let ingredients = file
            .ingredients
            .into_iter()
            .filter_map(|line| line.expected)
            .map(|expected| ParsedIngredient {
                item: expected.item,
                measurements: expected.measurements,
                note: expected.note,
                raw: None,
                section: expected.section,
            })
            .collect();
        recipes.push(Recipe {
            servings: None,
            ingredients,
        });
    }
    Ok(Corpus {
        name: name.to_string(),
        recipes,
    })
}

fn load_snapshot_corpus(root: &Path) -> Result<Corpus> {
    let mut recipes = Vec::new();
    for path in sorted_json_files(&root.join(SNAPSHOTS_DIR))? {
        let recipe: FinalRecipe = read_json(&path)?;
        recipes.push(Recipe {
            servings: recipe.servings,
            ingredients: recipe.ingredients,
        });
    }
    Ok(Corpus {
        name: "Pipeline snapshots".to_string(),
        recipes,
    })
}

/// The newest run under `runs_dir`, as written by `make pipeline`.
fn load_pipeline_run_corpus(runs_dir: &Path) -> Result<Corpus> {
    let mut runs = fs::read_dir(runs_dir)
        .with_context(|| format!("Failed to read {}", runs_dir.display()))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    runs.retain(|path| path.is_dir());
    runs.sort();
    let run_dir = runs
        .pop()
        .with_context(|| format!("No pipeline runs in {}", runs_dir.display()))?;

    // A running or failed run can be missing outputs, which would make
    // coverage look like it changed when only the corpus did.
    let manifest: RunManifest = read_json(&run_dir.join("manifest.json"))?;
    anyhow::ensure!(
        manifest.status == "completed",
        "Latest pipeline run {} has status {:?}, not \"completed\"; rerun `make pipeline` \
         or point RUNS_DIR at a directory whose newest run completed",
        run_dir.display(),
        manifest.status
    );

    // In a completed run, a URL without parse output is one whose fetch or
    // extraction failed, so it has no ingredients to audit.
    let urls_dir = run_dir.join("urls");
    let mut url_dirs = fs::read_dir(&urls_dir)
        .with_context(|| format!("Failed to read {}", urls_dir.display()))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    url_dirs.sort();

    let mut recipes = Vec::new();
    for url_dir in url_dirs {
        let output_path = url_dir.join("parse_ingredients").join("output.json");
        if !output_path.exists() {
            continue;
        }
        let output: ParseIngredientsOutput = read_json(&output_path)?;
        recipes.push(Recipe {
            servings: None,
            ingredients: output.ingredients,
        });
    }
    Ok(Corpus {
        name: format!("Pipeline run {}", run_dir.display()),
        recipes,
    })
}

fn load_prod_corpus(path: &Path) -> Result<Corpus> {
    let recipes: Vec<ProdRecipe> = read_json(path)?;
    Ok(Corpus {
        name: format!("Prod recipes {}", path.display()),
        recipes: recipes
            .into_iter()
            .map(|r| Recipe {
                servings: r.servings,
                ingredients: r.ingredients,
            })
            .collect(),
    })
}

fn audit_recipes(corpus: &Corpus) -> Result<RecipeCorpusStats> {
    let mut stats = RecipeCorpusStats::default();
    for recipe in &corpus.recipes {
        if recipe.ingredients.is_empty() {
            continue;
        }
        stats.recipes += 1;
        stats.lines += recipe.ingredients.len();
        if recipe.servings.is_some() {
            stats.recipes_with_servings += 1;
        }

        let estimate = nutrition::estimate(&recipe.ingredients, recipe.servings.as_deref(), 1.0)
            .map_err(|e| anyhow::anyhow!("Calorie estimate failed in {}: {e}", corpus.name))?;
        let unknown = &estimate.unknown_ingredients;
        stats.nutrition_computed += recipe.ingredients.len() - unknown.len();
        stats.nutrition_name_recognized += recipe.ingredients.len()
            - unknown
                .iter()
                .filter(|u| NAME_FAILURES.contains(&u.reason.as_str()))
                .count();
        if unknown.is_empty() {
            stats.recipes_fully_estimated += 1;
            if estimate.per_serving_calories.is_some() {
                stats.recipes_with_per_serving += 1;
            }
        }
        for u in unknown {
            *stats.nutrition_reasons.entry(u.reason.clone()).or_default() += 1;
            if NAME_FAILURES.contains(&u.reason.as_str()) {
                *stats
                    .nutrition_unrecognized
                    .entry(normalize_name(&u.item))
                    .or_default() += 1;
            }
        }

        for ingredient in &recipe.ingredients {
            let has_volume = ingredient
                .measurements
                .iter()
                .any(|m| is_volume_unit(m.unit.as_deref()));
            if has_volume {
                stats.volume_lines += 1;
                if find_density(&ingredient.item).is_some() {
                    stats.density_hits += 1;
                } else {
                    *stats
                        .density_misses
                        .entry(normalize_name(&ingredient.item))
                        .or_default() += 1;
                }
            }

            if categorize(&ingredient.item) == "Other" {
                *stats
                    .uncategorized
                    .entry(normalize_name(&ingredient.item))
                    .or_default() += 1;
            } else {
                stats.categorized += 1;
            }
        }
    }
    Ok(stats)
}

/// Whether nutrition recognizes the name alone, independent of any quantity.
fn nutrition_recognizes(item: &str) -> Result<bool> {
    let ingredient = ParsedIngredient {
        item: item.to_string(),
        measurements: vec![],
        note: None,
        raw: None,
        section: None,
    };
    let estimate = nutrition::estimate(std::slice::from_ref(&ingredient), None, 1.0)
        .map_err(|e| anyhow::anyhow!("Calorie estimate failed for {item:?}: {e}"))?;
    Ok(!estimate
        .unknown_ingredients
        .iter()
        .any(|u| NAME_FAILURES.contains(&u.reason.as_str())))
}

fn audit_shopping(items: &[ShoppingItem]) -> Result<ShoppingStats> {
    let mut stats = ShoppingStats::default();
    for entry in items {
        stats.items += 1;
        stats.uses += entry.count;
        if nutrition_recognizes(&entry.item)? {
            stats.nutrition_name_recognized += 1;
            stats.nutrition_name_recognized_uses += entry.count;
        } else {
            *stats
                .nutrition_unrecognized
                .entry(normalize_name(&entry.item))
                .or_default() += 1;
        }
        if categorize(&entry.item) == "Other" {
            *stats
                .uncategorized
                .entry(normalize_name(&entry.item))
                .or_default() += 1;
        } else {
            stats.categorized += 1;
            stats.categorized_uses += entry.count;
        }
    }
    Ok(stats)
}

fn pct(part: impl Into<u64>, whole: impl Into<u64>) -> String {
    let (part, whole) = (part.into(), whole.into());
    if whole == 0 {
        return "n/a".to_string();
    }
    format!("{:.1}%", part as f64 * 100.0 / whole as f64)
}

/// Exact counts alongside the rounded percentage, so a change of a single
/// match still shows up in the committed report.
fn share(part: u64, whole: u64) -> String {
    format!("{part}/{whole} ({})", pct(part, whole))
}

/// Most frequent names first, ties broken alphabetically.
fn top_names(counts: &HashMap<String, usize>, limit: usize) -> Vec<(&str, usize)> {
    let mut sorted: Vec<_> = counts.iter().map(|(n, c)| (n.as_str(), *c)).collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    sorted.truncate(limit);
    sorted
}

fn write_top_names(out: &mut String, title: &str, unit: &str, counts: &HashMap<String, usize>) {
    let total: usize = counts.values().sum();
    let _ = writeln!(
        out,
        "\n#### {title} ({} distinct, {total} {unit})\n",
        counts.len()
    );
    if counts.is_empty() {
        out.push_str("None.\n");
        return;
    }
    out.push_str("| Count | Name |\n| ---: | --- |\n");
    for (name, count) in top_names(counts, TOP_NAMES) {
        let _ = writeln!(out, "| {count} | {} |", name.replace('|', "\\|"));
    }
}

/// A short, stable hash of which names a matcher missed and how often. It
/// changes whenever the set changes, even when the totals stay the same.
fn fingerprint(counts: &HashMap<String, usize>) -> String {
    let mut entries: Vec<_> = counts.iter().collect();
    entries.sort();
    let mut hasher = Sha256::new();
    for (name, count) in entries {
        hasher.update(format!("{name}\t{count}\n").as_bytes());
    }
    hasher.finalize()[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn write_fingerprints(out: &mut String, matchers: &[(&str, &HashMap<String, usize>)]) {
    out.push_str(
        "\n### Unrecognized-name fingerprints\n\n\
         | Matcher | Distinct names | Fingerprint |\n| --- | ---: | --- |\n",
    );
    for (matcher, counts) in matchers {
        let _ = writeln!(
            out,
            "| {matcher} | {} | `{}` |",
            counts.len(),
            fingerprint(counts)
        );
    }
}

fn render_recipe_corpora(
    out: &mut String,
    corpora: &[(String, RecipeCorpusStats)],
    with_names: bool,
) {
    out.push_str(
        "## Summary\n\n\
         | Corpus | Recipes | Lines | Nutrition name recognized | Calories computed | \
         Recipes fully estimated | Recipes with per-serving | Volume lines with density | \
         Lines categorized |\n\
         | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n",
    );
    for (name, s) in corpora {
        let per_serving = if s.recipes_with_servings == 0 {
            "n/a (no servings)".to_string()
        } else {
            share(s.recipes_with_per_serving as u64, s.recipes as u64)
        };
        let _ = writeln!(
            out,
            "| {name} | {} | {} | {} | {} | {} | {per_serving} | {} | {} |",
            s.recipes,
            s.lines,
            share(s.nutrition_name_recognized as u64, s.lines as u64),
            share(s.nutrition_computed as u64, s.lines as u64),
            share(s.recipes_fully_estimated as u64, s.recipes as u64),
            share(s.density_hits as u64, s.volume_lines as u64),
            share(s.categorized as u64, s.lines as u64),
        );
    }

    for (name, s) in corpora {
        let _ = writeln!(out, "\n## {name}\n\n### Nutrition failure reasons\n");
        out.push_str("| Reason | Lines | Share of lines |\n| --- | ---: | ---: |\n");
        for (reason, count) in &s.nutrition_reasons {
            let _ = writeln!(
                out,
                "| {reason} | {count} | {} |",
                pct(*count as u64, s.lines as u64)
            );
        }
        write_fingerprints(
            out,
            &[
                ("Nutrition", &s.nutrition_unrecognized),
                ("Density", &s.density_misses),
                ("Shopping category", &s.uncategorized),
            ],
        );
        if !with_names {
            continue;
        }
        let _ = writeln!(out, "\n### Top unrecognized names (top {TOP_NAMES})");
        write_top_names(out, "Nutrition", "lines", &s.nutrition_unrecognized);
        write_top_names(
            out,
            "Density (volume lines only)",
            "lines",
            &s.density_misses,
        );
        write_top_names(
            out,
            "Shopping category (\"Other\")",
            "lines",
            &s.uncategorized,
        );
    }
}

fn render_shopping(out: &mut String, s: &ShoppingStats, with_names: bool) {
    out.push_str(
        "\n## Shopping-list corpus\n\n\
         Hand-typed shopping-list items from prod (`data/shopping-list-categories.json`). \
         Usage-weighted numbers count each item by how often it was added.\n\n\
         | Metric | Distinct items | Usage-weighted |\n| --- | ---: | ---: |\n",
    );
    let _ = writeln!(out, "| Items | {} | {} |", s.items, s.uses);
    let _ = writeln!(
        out,
        "| Nutrition name recognized | {} | {} |",
        share(s.nutrition_name_recognized as u64, s.items as u64),
        share(s.nutrition_name_recognized_uses, s.uses)
    );
    let _ = writeln!(
        out,
        "| Categorized (not \"Other\") | {} | {} |",
        share(s.categorized as u64, s.items as u64),
        share(s.categorized_uses, s.uses)
    );
    write_fingerprints(
        out,
        &[
            ("Nutrition", &s.nutrition_unrecognized),
            ("Shopping category", &s.uncategorized),
        ],
    );
    if !with_names {
        return;
    }
    let _ = writeln!(out, "\n### Top unrecognized names (top {TOP_NAMES})");
    write_top_names(out, "Nutrition", "items", &s.nutrition_unrecognized);
    write_top_names(
        out,
        "Shopping category (\"Other\")",
        "items",
        &s.uncategorized,
    );
}

const HEADER: &str = "# Ingredient catalog audit\n\n\
Generated by `make ingredient-catalog-audit` (also run by `make pipeline`). Do not edit by hand.\n\n\
Replays the three ingredient-name matchers over committed corpora:\n\n\
- **Nutrition**: `nutrition::estimate`. \"Name recognized\" means the name matched a food; \
\"calories computed\" also needs a usable quantity. A recipe is fully estimated when every line \
has calories computed, and has per-serving calories when it is also fully estimated and its \
servings parse; only snapshots carry servings.\n\
- **Density**: `find_density`, over lines with a volume unit.\n\
- **Shopping category**: `categorize`; \"categorized\" means not \"Other\".\n\n\
A CLI unit test regenerates this file and fails if it is stale. The most frequent unrecognized \
names change with every catalog edit, so they are written to the uncommitted \
`logs/ingredient-catalog-audit-local.md` instead.\n\n";

const LOCAL_HEADER: &str = "# Ingredient catalog audit (local, not committed)\n\n\
Written by `make ingredient-catalog-audit`. Same metrics as `data/ingredient-catalog-audit.md`, \
plus the most frequent unrecognized names per matcher, and any `RUNS_DIR` / `PROD_RECIPES` corpora.\n\n";

struct CommittedAudit {
    recipes: Vec<(String, RecipeCorpusStats)>,
    shopping: ShoppingStats,
}

fn audit_committed(root: &Path) -> Result<CommittedAudit> {
    let corpora = [
        load_fixture_corpus(root, "pipeline", "Pipeline fixtures")?,
        load_fixture_corpus(root, "paprika", "Paprika fixtures")?,
        load_snapshot_corpus(root)?,
    ];
    let recipes = corpora
        .iter()
        .map(|corpus| Ok((corpus.name.clone(), audit_recipes(corpus)?)))
        .collect::<Result<Vec<_>>>()?;
    let shopping_items: Vec<ShoppingItem> = read_json(&root.join(SHOPPING_CORPUS))?;
    let shopping = audit_shopping(&shopping_items)?;
    Ok(CommittedAudit { recipes, shopping })
}

/// Only aggregate numbers are committed; name lists shift with every catalog
/// change and live in the local report instead.
fn render_committed(audit: &CommittedAudit) -> String {
    let mut out = HEADER.to_string();
    render_recipe_corpora(&mut out, &audit.recipes, false);
    render_shopping(&mut out, &audit.shopping, false);
    out
}

pub fn run(root: &Path, runs_dir: Option<&Path>, prod_recipes: Option<&Path>) -> Result<()> {
    let mut committed = audit_committed(root)?;
    let committed_path = root.join(COMMITTED_REPORT);
    fs::write(&committed_path, render_committed(&committed))
        .with_context(|| format!("Failed to write {}", committed_path.display()))?;
    tracing::info!("Ingredient catalog audit saved to: {COMMITTED_REPORT}");

    let mut local = Vec::new();
    if let Some(runs_dir) = runs_dir {
        local.push(load_pipeline_run_corpus(runs_dir)?);
    }
    if let Some(path) = prod_recipes {
        local.push(load_prod_corpus(path)?);
    }
    for corpus in &local {
        committed
            .recipes
            .push((corpus.name.clone(), audit_recipes(corpus)?));
    }
    let mut out = LOCAL_HEADER.to_string();
    render_recipe_corpora(&mut out, &committed.recipes, true);
    render_shopping(&mut out, &committed.shopping, true);
    let local_path = root.join(LOCAL_REPORT);
    fs::create_dir_all(root.join("logs"))?;
    fs::write(&local_path, out)
        .with_context(|| format!("Failed to write {}", local_path.display()))?;
    tracing::info!("Local audit with unrecognized names saved to: {LOCAL_REPORT}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ingredient(item: &str, amount: Option<&str>, unit: Option<&str>) -> ParsedIngredient {
        ParsedIngredient {
            item: item.to_string(),
            measurements: vec![Measurement {
                amount: amount.map(str::to_string),
                unit: unit.map(str::to_string),
            }],
            note: None,
            raw: None,
            section: None,
        }
    }

    #[test]
    fn recipe_stats_split_name_and_quantity_failures() {
        let corpus = Corpus {
            name: "test".to_string(),
            recipes: vec![
                Recipe {
                    servings: Some("2".to_string()),
                    ingredients: vec![
                        ingredient("granulated sugar", Some("1"), Some("cup")),
                        ingredient("Moon  Dust", Some("1"), Some("cup")),
                        ingredient("eggs", Some("2"), Some("large")),
                    ],
                },
                Recipe {
                    servings: Some("4".to_string()),
                    ingredients: vec![ingredient("granulated sugar", Some("100"), Some("g"))],
                },
                Recipe {
                    servings: None,
                    ingredients: vec![],
                },
            ],
        };
        let stats = audit_recipes(&corpus).unwrap();

        assert_eq!(stats.recipes, 2, "empty recipes are skipped");
        assert_eq!(stats.lines, 4);
        assert_eq!(stats.nutrition_computed, 2);
        assert_eq!(
            stats.nutrition_name_recognized, 3,
            "eggs match but lack a unit"
        );
        assert_eq!(stats.recipes_fully_estimated, 1);
        assert_eq!(stats.recipes_with_per_serving, 1);
        assert_eq!(
            stats.nutrition_unrecognized.get("moon dust"),
            Some(&1),
            "names are grouped case- and whitespace-insensitively"
        );
        assert_eq!(stats.nutrition_reasons["Unsupported quantity unit"], 1);
        assert_eq!(stats.volume_lines, 2);
        assert_eq!(stats.density_hits, 1);
        assert_eq!(stats.density_misses.get("moon dust"), Some(&1));
    }

    #[test]
    fn shopping_stats_weight_by_usage() {
        let items = vec![
            ShoppingItem {
                item: "granulated sugar".to_string(),
                count: 3,
            },
            ShoppingItem {
                item: "moon dust".to_string(),
                count: 1,
            },
        ];
        let stats = audit_shopping(&items).unwrap();
        assert_eq!(stats.items, 2);
        assert_eq!(stats.uses, 4);
        assert_eq!(stats.nutrition_name_recognized, 1);
        assert_eq!(stats.nutrition_name_recognized_uses, 3);
        assert_eq!(
            pct(stats.nutrition_name_recognized_uses, stats.uses),
            "75.0%"
        );
    }

    #[test]
    fn top_names_sort_by_count_then_name() {
        let counts = HashMap::from([
            ("b".to_string(), 2),
            ("a".to_string(), 2),
            ("c".to_string(), 5),
        ]);
        assert_eq!(top_names(&counts, 2), vec![("c", 5), ("a", 2)]);
        assert_eq!(pct(0u64, 0u64), "n/a");
        assert_eq!(share(1, 60_000), "1/60000 (0.0%)");
    }

    #[test]
    fn fingerprint_tracks_names_not_just_totals() {
        let before = HashMap::from([("salt".to_string(), 2), ("water".to_string(), 1)]);
        let reordered = HashMap::from([("water".to_string(), 1), ("salt".to_string(), 2)]);
        let swapped = HashMap::from([("salt".to_string(), 2), ("pepper".to_string(), 1)]);
        assert_eq!(fingerprint(&before), fingerprint(&reordered));
        assert_ne!(
            fingerprint(&before),
            fingerprint(&swapped),
            "same totals, different names"
        );
        assert_eq!(fingerprint(&before).len(), 12);
    }

    fn write_run(runs_dir: &Path, run_id: &str, status: &str) {
        let run_dir = runs_dir.join(run_id);
        let parse_dir = run_dir.join("urls/example-com_soup/parse_ingredients");
        fs::create_dir_all(&parse_dir).unwrap();
        fs::write(
            run_dir.join("manifest.json"),
            format!(r#"{{"status": "{status}"}}"#),
        )
        .unwrap();
        fs::write(
            parse_dir.join("output.json"),
            r#"{"ingredients": [{"item": "water", "measurements": [], "note": null, "raw": null, "section": null}]}"#,
        )
        .unwrap();
        fs::create_dir_all(run_dir.join("urls/example-com_blocked")).unwrap();
    }

    #[test]
    fn pipeline_run_must_be_completed() {
        let runs = tempfile::tempdir().unwrap();
        write_run(runs.path(), "2026-01-01_00-00-00", "completed");
        let corpus = load_pipeline_run_corpus(runs.path()).unwrap();
        assert_eq!(
            corpus.recipes.len(),
            1,
            "URLs without parse output (failed fetch/extract) are skipped"
        );

        write_run(runs.path(), "2026-01-02_00-00-00", "failed");
        let error = load_pipeline_run_corpus(runs.path())
            .err()
            .expect("the newest run failed")
            .to_string();
        assert!(error.contains("\"failed\""), "{error}");
    }

    #[test]
    fn committed_report_is_current() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let expected = render_committed(&audit_committed(&root).unwrap());
        let committed = fs::read_to_string(root.join(COMMITTED_REPORT)).unwrap();
        assert!(
            committed == expected,
            "{COMMITTED_REPORT} is stale; run `make ingredient-catalog-audit` and commit the result"
        );
    }
}
