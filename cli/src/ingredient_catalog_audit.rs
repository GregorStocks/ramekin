//! Ingredient catalog audit.
//!
//! Replays the ingredient-name matchers (nutrition, density, shopping category)
//! over recipe corpora and reports how much of each corpus they recognize, so
//! catalog changes show measurable before/after numbers.

use anyhow::{Context, Result};
use ramekin_core::catalog::{
    is_non_food, is_non_food_with, is_volume_unit, line_grams_per_cup_with, resolve, Learned,
    LearnedTarget, Resolution,
};
use ramekin_core::final_recipe::FinalRecipe;
use ramekin_core::ingredient_categorizer::{categorize, categorize_with};
use ramekin_core::ingredient_parser::{Measurement, ParsedIngredient};
use ramekin_core::nutrition;
use ramekin_core::types::ParseIngredientsOutput;
use serde::{Deserialize, Serialize};
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
const UNRESOLVED_QUEUE: &str = "logs/catalog-unresolved.json";
const MAX_EXAMPLES: usize = 3;
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
    /// Names the server learned (catalog step 3); empty for committed corpora.
    learned: Learned,
    /// Weights the server estimated (catalog step 3); empty for committed corpora.
    weights: nutrition::Weights,
}

#[derive(Default)]
struct RecipeCorpusStats {
    recipes: usize,
    recipes_with_servings: usize,
    /// Headers, serving notes, and products: neither recognized foods nor unknown.
    non_food: usize,
    lines: usize,
    nutrition_name_recognized: usize,
    nutrition_computed: usize,
    recipes_fully_estimated: usize,
    recipes_with_per_serving: usize,
    /// Recipes by how many ingredients the estimate couldn't count, bucketed
    /// as in `UNKNOWN_BUCKETS`; this is what `MAX_UNKNOWN_LINES` is chosen from.
    unknown_lines_per_recipe: [usize; UNKNOWN_BUCKETS.len()],
    /// Recipes by estimate status (complete, partial, insufficient, empty).
    statuses: BTreeMap<String, usize>,
    nutrition_reasons: BTreeMap<String, usize>,
    nutrition_unrecognized: HashMap<String, usize>,
    /// Every nutrition failure keyed by "reason: name", so the fingerprint
    /// changes when a line moves between failure reasons.
    nutrition_failures: HashMap<String, usize>,
    volume_lines: usize,
    density_hits: usize,
    density_misses: HashMap<String, usize>,
    categorized: usize,
    uncategorized: HashMap<String, usize>,
    /// Distinct weights uncounted lines need that the corpus's estimated
    /// weights don't have: what the server would ask the model for.
    weight_gaps: std::collections::BTreeSet<nutrition::WeightKey>,
}

/// Buckets of uncounted ingredients per recipe: (label, lowest count).
const UNKNOWN_BUCKETS: [(&str, usize); 6] = [
    ("0", 0),
    ("1", 1),
    ("2", 2),
    ("3", 3),
    ("4-5", 4),
    ("6+", 6),
];

fn unknown_bucket(unknown: usize) -> usize {
    UNKNOWN_BUCKETS
        .iter()
        .rposition(|(_, lowest)| unknown >= *lowest)
        .expect("the first bucket starts at 0")
}

#[derive(Default)]
struct ShoppingStats {
    items: usize,
    uses: u64,
    nutrition_name_recognized: usize,
    nutrition_name_recognized_uses: u64,
    /// Products (dish soap), excluded from nutrition coverage.
    non_food: usize,
    non_food_uses: u64,
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
    raw: String,
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
            .filter_map(|line| Some((line.raw, line.expected?)))
            .map(|(raw, expected)| ParsedIngredient {
                item: expected.item,
                measurements: expected.measurements,
                note: expected.note,
                raw: Some(raw),
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
        learned: Learned::new(),
        weights: nutrition::Weights::new(),
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
        learned: Learned::new(),
        weights: nutrition::Weights::new(),
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
        learned: Learned::new(),
        weights: nutrition::Weights::new(),
    })
}

/// A row of the server's `ingredient_name_resolutions` table, as exported
/// for `--learned`.
#[derive(Deserialize)]
struct LearnedRow {
    name: String,
    status: String,
    disposition: Option<String>,
    catalog_key: Option<String>,
}

/// The resolved rows of a learned-names export, as the server reads them.
fn load_learned(path: &Path) -> Result<Learned> {
    let rows: Vec<LearnedRow> = read_json(path)?;
    rows.into_iter()
        .filter(|row| row.status == "resolved")
        .map(|row| {
            let target = match (row.disposition.as_deref(), row.catalog_key) {
                (Some("entry"), Some(key)) => LearnedTarget::Entry(key),
                (Some("not_food"), None) => LearnedTarget::NotFood,
                (Some("unknown"), None) => LearnedTarget::Unknown,
                (disposition, key) => anyhow::bail!(
                    "Bad learned row for {:?}: disposition {disposition:?}, key {key:?}",
                    row.name
                ),
            };
            Ok((row.name, target))
        })
        .collect()
}

/// A row of the server's `ingredient_weight_estimates` table, as exported
/// for `--weights`.
#[derive(Deserialize)]
struct WeightRow {
    food: String,
    unit: String,
    status: String,
    grams: Option<f64>,
}

/// The estimated weights in a weights export, as the server reads them.
fn load_weights(path: &Path) -> Result<nutrition::Weights> {
    let rows: Vec<WeightRow> = read_json(path)?;
    Ok(rows
        .into_iter()
        .filter(|row| row.status == "resolved")
        .filter_map(|row| {
            row.grams.map(|grams| {
                (
                    nutrition::WeightKey {
                        food: row.food,
                        unit: row.unit,
                    },
                    grams,
                )
            })
        })
        .collect())
}

fn load_prod_corpus(path: &Path, learned: Option<&Path>, weights: Option<&Path>) -> Result<Corpus> {
    let recipes: Vec<ProdRecipe> = read_json(path)?;
    let mut name = format!("Prod recipes {}", path.display());
    if learned.is_some() {
        name.push_str(" + learned names");
    }
    if weights.is_some() {
        name.push_str(" + estimated weights");
    }
    let learned = learned.map(load_learned).transpose()?.unwrap_or_default();
    let weights = weights.map(load_weights).transpose()?.unwrap_or_default();
    Ok(Corpus {
        name,
        recipes: recipes
            .into_iter()
            .map(|r| Recipe {
                servings: r.servings,
                ingredients: r.ingredients,
            })
            .collect(),
        learned,
        weights,
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

        let estimate = nutrition::estimate_with(
            &recipe.ingredients,
            recipe.servings.as_deref(),
            1.0,
            &corpus.learned,
            &corpus.weights,
        )
        .map_err(|e| anyhow::anyhow!("Calorie estimate failed in {}: {e}", corpus.name))?;
        // Only unknown lines decide the status; lines given no amount are left
        // out but still not computed.
        stats.unknown_lines_per_recipe[unknown_bucket(estimate.unknown_ingredients.len())] += 1;
        let unknown: Vec<_> = estimate
            .unknown_ingredients
            .iter()
            .chain(&estimate.no_amount)
            .collect();
        *stats
            .statuses
            .entry(format!("{:?}", estimate.status))
            .or_default() += 1;
        let non_food = recipe
            .ingredients
            .iter()
            .filter(|ingredient| is_non_food_with(&ingredient.item, &corpus.learned))
            .count();
        stats.non_food += non_food;
        stats.nutrition_computed += recipe.ingredients.len() - unknown.len() - non_food;
        stats.nutrition_name_recognized += recipe.ingredients.len()
            - non_food
            - unknown
                .iter()
                .filter(|u| NAME_FAILURES.contains(&u.reason.as_str()))
                .count();
        stats
            .weight_gaps
            .extend(estimate.weight_gaps.iter().cloned());
        if unknown.is_empty() {
            stats.recipes_fully_estimated += 1;
            if estimate.per_serving_calories.is_some() {
                stats.recipes_with_per_serving += 1;
            }
        }
        for u in &unknown {
            *stats.nutrition_reasons.entry(u.reason.clone()).or_default() += 1;
            *stats
                .nutrition_failures
                .entry(format!("{}: {}", u.reason, normalize_name(&u.item)))
                .or_default() += 1;
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
                if line_grams_per_cup_with(
                    &ingredient.item,
                    ingredient.note.as_deref(),
                    &corpus.learned,
                )
                .is_some()
                {
                    stats.density_hits += 1;
                } else {
                    *stats
                        .density_misses
                        .entry(normalize_name(&ingredient.item))
                        .or_default() += 1;
                }
            }

            if categorize_with(&ingredient.item, &corpus.learned) == "Other" {
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
        .chain(&estimate.no_amount)
        .any(|u| NAME_FAILURES.contains(&u.reason.as_str())))
}

fn audit_shopping(items: &[ShoppingItem]) -> Result<ShoppingStats> {
    let mut stats = ShoppingStats::default();
    for entry in items {
        stats.items += 1;
        stats.uses += entry.count;
        if is_non_food(&entry.item) {
            stats.non_food += 1;
            stats.non_food_uses += entry.count;
        } else if nutrition_recognizes(&entry.item)? {
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
         | Matcher | Distinct entries | Fingerprint |\n| --- | ---: | --- |\n",
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
         | Corpus | Recipes | Lines | Non-food lines | Nutrition name recognized | \
         Calories computed | Recipes fully estimated | Recipes with per-serving | \
         Volume lines with density | Lines categorized |\n\
         | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n",
    );
    for (name, s) in corpora {
        // Nutrition coverage is over lines that name something eaten.
        let food_lines = (s.lines - s.non_food) as u64;
        let per_serving = if s.recipes_with_servings == 0 {
            "n/a (no servings)".to_string()
        } else {
            share(s.recipes_with_per_serving as u64, s.recipes as u64)
        };
        let _ = writeln!(
            out,
            "| {name} | {} | {} | {} | {} | {} | {} | {per_serving} | {} | {} |",
            s.recipes,
            s.lines,
            s.non_food,
            share(s.nutrition_name_recognized as u64, food_lines),
            share(s.nutrition_computed as u64, food_lines),
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
        let _ = writeln!(
            out,
            "\n### Uncounted ingredients per recipe\n\n\
             | Uncounted | Recipes | Share of recipes |\n| --- | ---: | ---: |"
        );
        for ((label, _), count) in UNKNOWN_BUCKETS.iter().zip(s.unknown_lines_per_recipe) {
            let _ = writeln!(
                out,
                "| {label} | {count} | {} |",
                pct(count as u64, s.recipes as u64)
            );
        }
        let _ = writeln!(
            out,
            "\nEstimate status with at most {} uncounted: {}",
            nutrition::MAX_UNKNOWN_LINES,
            s.statuses
                .iter()
                .map(|(status, count)| format!("{status} {}", pct(*count as u64, s.recipes as u64)))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let _ = writeln!(
            out,
            "\nWeights to estimate (distinct food and unit pairs the catalog can't weigh): {}",
            s.weight_gaps.len()
        );
        write_fingerprints(
            out,
            &[
                ("Nutrition (all failures)", &s.nutrition_failures),
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
            "Nutrition failures (reason: name)",
            "lines",
            &s.nutrition_failures,
        );
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
        share(
            s.nutrition_name_recognized as u64,
            (s.items - s.non_food) as u64
        ),
        share(s.nutrition_name_recognized_uses, s.uses - s.non_food_uses)
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
\"calories computed\" also needs a usable quantity, and includes negligible lines (salt, spices \
to taste). Non-food lines (leftover headers, serving notes, products like parchment paper) are \
counted separately and excluded from both, numerator and denominator. A recipe is fully estimated when every line \
has calories computed, and has per-serving calories when it is also fully estimated and its \
servings parse; only snapshots carry servings.\n\
- **Density**: `catalog::grams_per_cup`, over lines with a volume unit.\n\
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

pub fn run(
    root: &Path,
    runs_dir: Option<&Path>,
    prod_recipes: Option<&Path>,
    learned: Option<&Path>,
    weights: Option<&Path>,
) -> Result<()> {
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
        local.push(load_prod_corpus(path, learned, weights)?);
    } else if learned.is_some() || weights.is_some() {
        anyhow::bail!("--learned and --weights need --prod-recipes");
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

const CURATED_PATH: &str = "ramekin-core/src/catalog/data/curated.json";
const CLEANUP_REPORT: &str = "logs/catalog-alias-cleanup.md";

/// Re-key or remove curated names the parser no longer produces, writing
/// curated.json and a report of every change and conflict.
pub fn clean_aliases(root: &Path) -> Result<()> {
    use ramekin_core::catalog::{clean_curated, CuratedChange};
    let path = root.join(CURATED_PATH);
    let json =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {CURATED_PATH}"))?;
    let (cleaned, changes) = clean_curated(&json);
    fs::write(&path, cleaned).with_context(|| format!("Failed to write {CURATED_PATH}"))?;
    let (mut removed, mut renamed, mut conflicts) = (Vec::new(), Vec::new(), Vec::new());
    for change in &changes {
        match change {
            CuratedChange::Remove { section, name } => removed.push(format!("- {section}: {name}")),
            CuratedChange::Rename { section, from, to } => {
                renamed.push(format!("- {section}: {from} -> {to}"))
            }
            CuratedChange::Conflict {
                section,
                name,
                parsed,
                detail,
            } => conflicts.push(format!(
                "- {section}: {name} (parses to {parsed}: {detail})"
            )),
        }
    }
    let report = format!(
        "# Curated alias cleanup\n\n## Removed ({})\n\n{}\n\n## Renamed ({})\n\n{}\n\n## Conflicts, kept ({})\n\n{}\n",
        removed.len(),
        removed.join("\n"),
        renamed.len(),
        renamed.join("\n"),
        conflicts.len(),
        conflicts.join("\n"),
    );
    fs::create_dir_all(root.join("logs"))?;
    fs::write(root.join(CLEANUP_REPORT), report)?;
    tracing::info!(
        "Removed {}, renamed {}, kept {} conflicts; report at {CLEANUP_REPORT}",
        removed.len(),
        renamed.len(),
        conflicts.len()
    );
    Ok(())
}

/// One name the catalog doesn't resolve, as a classification work item.
#[derive(Serialize)]
struct UnresolvedName {
    name: String,
    count: usize,
    /// Lines per corpus ("pipeline", "paprika", "prod").
    corpora: BTreeMap<String, usize>,
    /// Up to three raw source lines, for context.
    examples: Vec<String>,
}

/// Collect every name `resolve` leaves `Unresolved`, most frequent first.
/// Ambiguous names are deliberate and excluded. Snapshots are skipped because
/// they are a subset of the pipeline fixtures.
fn unresolved_names(corpora: &[(&str, Corpus)]) -> Vec<UnresolvedName> {
    let mut names: HashMap<String, UnresolvedName> = HashMap::new();
    for (key, corpus) in corpora {
        for ingredient in corpus.recipes.iter().flat_map(|recipe| &recipe.ingredients) {
            if !matches!(resolve(&ingredient.item), Resolution::Unresolved) {
                continue;
            }
            let name = normalize_name(&ingredient.item);
            let entry = names.entry(name.clone()).or_insert_with(|| UnresolvedName {
                name,
                count: 0,
                corpora: BTreeMap::new(),
                examples: Vec::new(),
            });
            entry.count += 1;
            *entry.corpora.entry(key.to_string()).or_default() += 1;
            let example = ingredient
                .raw
                .clone()
                .unwrap_or_else(|| ingredient.item.clone());
            if entry.examples.len() < MAX_EXAMPLES && !entry.examples.contains(&example) {
                entry.examples.push(example);
            }
        }
    }
    let mut names: Vec<_> = names.into_values().collect();
    names.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    names
}

/// Write the classification work queue (`make ingredient-catalog-unresolved`).
pub fn export_unresolved(root: &Path, prod_recipes: Option<&Path>) -> Result<()> {
    let mut corpora = vec![
        (
            "pipeline",
            load_fixture_corpus(root, "pipeline", "Pipeline fixtures")?,
        ),
        (
            "paprika",
            load_fixture_corpus(root, "paprika", "Paprika fixtures")?,
        ),
    ];
    if let Some(path) = prod_recipes {
        corpora.push(("prod", load_prod_corpus(path, None, None)?));
    }
    let names = unresolved_names(&corpora);
    let path = root.join(UNRESOLVED_QUEUE);
    fs::create_dir_all(root.join("logs"))?;
    fs::write(&path, serde_json::to_string_pretty(&names)? + "\n")
        .with_context(|| format!("Failed to write {}", path.display()))?;
    tracing::info!(
        "{} unresolved names ({} lines) saved to: {UNRESOLVED_QUEUE}",
        names.len(),
        names.iter().map(|name| name.count).sum::<usize>()
    );
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
                        ingredient("kale", Some("2"), None),
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
            learned: Learned::new(),
            weights: nutrition::Weights::new(),
        };
        let stats = audit_recipes(&corpus).unwrap();

        assert_eq!(stats.recipes, 2, "empty recipes are skipped");
        assert_eq!(stats.lines, 4);
        assert_eq!(stats.nutrition_computed, 2);
        assert_eq!(
            stats.nutrition_name_recognized, 3,
            "kale matches but a bare count has no weight"
        );
        assert_eq!(stats.recipes_fully_estimated, 1);
        assert_eq!(stats.recipes_with_per_serving, 1);
        assert_eq!(
            stats.nutrition_unrecognized.get("moon dust"),
            Some(&1),
            "names are grouped case- and whitespace-insensitively"
        );
        assert_eq!(stats.nutrition_reasons["Unsupported quantity unit"], 1);
        assert_eq!(
            stats
                .nutrition_failures
                .get("Unsupported quantity unit: kale"),
            Some(&1),
            "quantity failures on recognized names are fingerprinted too"
        );
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
    fn unresolved_names_group_count_and_keep_examples() {
        let line = |item: &str, raw: &str| ParsedIngredient {
            item: item.to_string(),
            measurements: vec![],
            note: None,
            raw: Some(raw.to_string()),
            section: None,
        };
        let corpus = |ingredients| Corpus {
            name: "test".to_string(),
            recipes: vec![Recipe {
                servings: None,
                ingredients,
            }],
            learned: Learned::new(),
            weights: nutrition::Weights::new(),
        };
        let names = unresolved_names(&[
            (
                "pipeline",
                corpus(vec![
                    line("Moon Dust", "1 cup Moon Dust"),
                    line("moon  dust", "2 cups moon dust"),
                    line("granulated sugar", "1 cup granulated sugar"),
                    line("cheese", "1 cup cheese"),
                ]),
            ),
            ("prod", corpus(vec![line("moon dust", "1 cup Moon Dust")])),
        ]);
        assert_eq!(names.len(), 1, "resolved and ambiguous names are excluded");
        assert_eq!(names[0].name, "moon dust");
        assert_eq!(names[0].count, 3);
        assert_eq!(names[0].corpora["pipeline"], 2);
        assert_eq!(names[0].corpora["prod"], 1);
        assert_eq!(
            names[0].examples,
            vec!["1 cup Moon Dust", "2 cups moon dust"],
            "examples are deduplicated"
        );
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

    #[test]
    fn learned_names_count_in_the_prod_corpus() {
        let dir = tempfile::tempdir().unwrap();
        let recipes = dir.path().join("recipes.json");
        let learned = dir.path().join("learned.json");
        fs::write(
            &recipes,
            r#"[{"servings": null, "ingredients": [
                {"item": "zzqq sugar", "measurements": [{"amount": "100", "unit": "g"}]},
                {"item": "zzqq flour", "measurements": [{"amount": "1", "unit": "cup"}]},
                {"item": "zzqq garnish plate", "measurements": []}
            ]}]"#,
        )
        .unwrap();
        fs::write(
            &learned,
            r#"[
                {"name": "zzqq sugar", "status": "resolved", "disposition": "entry",
                 "catalog_key": "granulated sugar"},
                {"name": "zzqq flour", "status": "resolved", "disposition": "entry",
                 "catalog_key": "all-purpose flour"},
                {"name": "zzqq garnish plate", "status": "resolved", "disposition": "not_food",
                 "catalog_key": null},
                {"name": "pending one", "status": "pending", "disposition": null,
                 "catalog_key": null}
            ]"#,
        )
        .unwrap();
        let without = audit_recipes(&load_prod_corpus(&recipes, None, None).unwrap()).unwrap();
        assert_eq!(without.recipes_fully_estimated, 0);
        assert_eq!((without.non_food, without.density_hits), (0, 0));
        let corpus = load_prod_corpus(&recipes, Some(&learned), None).unwrap();
        assert_eq!(corpus.learned.len(), 3, "only resolved rows");
        let with = audit_recipes(&corpus).unwrap();
        assert_eq!(with.recipes_fully_estimated, 1);
        // Every metric reads learned names: the not-food line is non-food and
        // the learned flour has the flour density.
        assert_eq!((with.non_food, with.density_hits), (1, 1));

        fs::write(
            &learned,
            r#"[{"name": "x", "status": "resolved", "disposition": "entry", "catalog_key": null}]"#,
        )
        .unwrap();
        assert!(load_prod_corpus(&recipes, Some(&learned), None).is_err());
    }

    #[test]
    fn estimated_weights_count_in_the_prod_corpus() {
        let dir = tempfile::tempdir().unwrap();
        let recipes = dir.path().join("recipes.json");
        let weights = dir.path().join("weights.json");
        fs::write(
            &recipes,
            r#"[{"servings": null, "ingredients": [
                {"item": "capers", "measurements": [{"amount": "2", "unit": "tbsp"}]}
            ]}]"#,
        )
        .unwrap();
        let food = match ramekin_core::catalog::resolve("capers") {
            ramekin_core::catalog::Resolution::Entry { entry, .. } => entry.id.clone(),
            other => panic!("capers: {other:?}"),
        };
        fs::write(
            &weights,
            serde_json::json!([
                {"food": food, "unit": "cup", "status": "resolved", "grams": 136.0},
                {"food": food, "unit": "jar", "status": "pending", "grams": null}
            ])
            .to_string(),
        )
        .unwrap();
        let without = audit_recipes(&load_prod_corpus(&recipes, None, None).unwrap()).unwrap();
        assert_eq!(without.recipes_fully_estimated, 0);
        let corpus = load_prod_corpus(&recipes, None, Some(&weights)).unwrap();
        assert_eq!(corpus.weights.len(), 1, "only resolved rows with grams");
        assert_eq!(audit_recipes(&corpus).unwrap().recipes_fully_estimated, 1);
    }
}
