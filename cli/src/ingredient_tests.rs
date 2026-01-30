//! CLI commands for ingredient parsing test management.
//!
//! Provides commands to generate and update ingredient parsing test fixtures.
//! Test fixtures show output after each pipeline step (parse_ingredients, enrich_metric_weights).

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use ramekin_core::ingredient_parser::{parse_ingredient, Measurement, ParsedIngredient};
use ramekin_core::metric_weights::{add_metric_weight_alternative, EnrichmentStats};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

/// A test case for ingredient parsing (new format with step_outputs)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestCase {
    raw: String,
    step_outputs: HashMap<String, StepOutput>,
}

/// Output from a single step
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct StepOutput {
    item: String,
    measurements: Vec<Measurement>,
    note: Option<String>,
}

impl From<ParsedIngredient> for StepOutput {
    fn from(ing: ParsedIngredient) -> Self {
        Self {
            item: ing.item,
            measurements: ing.measurements,
            note: ing.note,
        }
    }
}

impl From<StepOutput> for ParsedIngredient {
    fn from(output: StepOutput) -> Self {
        Self {
            item: output.item,
            measurements: output.measurements,
            note: output.note,
            raw: None,
        }
    }
}

/// Legacy test case format for migration
#[derive(Debug, Clone, Deserialize)]
struct LegacyTestCase {
    raw: String,
    #[allow(dead_code)]
    expected: StepOutput,
}

/// Default path to the fixtures directory
fn default_fixtures_dir() -> PathBuf {
    PathBuf::from("ramekin-core/tests/fixtures/ingredient_parsing")
}

/// Run all pipeline steps on a raw ingredient string
fn run_pipeline(raw: &str) -> HashMap<String, StepOutput> {
    let mut outputs = HashMap::new();

    // Step 1: parse_ingredients
    let parsed = parse_ingredient(raw);
    outputs.insert(
        "parse_ingredients".to_string(),
        StepOutput::from(parsed.clone()),
    );

    // Step 2: enrich_metric_weights
    let mut stats = EnrichmentStats::default();
    let enriched = add_metric_weight_alternative(parsed, &mut stats);
    outputs.insert(
        "enrich_metric_weights".to_string(),
        StepOutput::from(enriched),
    );

    outputs
}

/// Generate test fixtures from the latest pipeline run.
///
/// Reads parse_ingredients output from pipeline results and creates
/// individual JSON test case files in the pipeline/ directory.
/// Uses UPSERT behavior: only adds new test cases, never deletes existing ones.
pub fn generate_from_pipeline(runs_dir: &Path, fixtures_dir: Option<&Path>) -> Result<()> {
    let fixtures_dir = fixtures_dir
        .map(PathBuf::from)
        .unwrap_or_else(default_fixtures_dir);
    let pipeline_dir = fixtures_dir.join("pipeline");

    // Find latest run
    let (run_id, run_dir) = find_latest_run(runs_dir)?;
    println!("Generating from run: {}", run_id);

    // Create directory if it doesn't exist (UPSERT: never delete existing)
    fs::create_dir_all(&pipeline_dir)?;

    let mut total_cases = 0;

    // Walk through all URL directories in the run
    let urls_dir = run_dir.join("urls");
    for entry in fs::read_dir(&urls_dir)? {
        let entry = entry?;
        let url_dir = entry.path();

        if !url_dir.is_dir() {
            continue;
        }

        // Look for parse_ingredients output
        let parse_output = url_dir.join("parse_ingredients").join("output.json");
        if !parse_output.exists() {
            continue;
        }

        // Read the parsed ingredients
        let content = fs::read_to_string(&parse_output)?;
        let output: ParseIngredientsOutput =
            serde_json::from_str(&content).context("Failed to parse output.json")?;

        // Extract site name from URL directory name
        let url_name = url_dir.file_name().unwrap().to_string_lossy();
        let site = extract_site_from_url(&url_name);
        let recipe_slug = extract_recipe_slug(&url_name);

        // Create test cases for each ingredient
        for (idx, ingredient) in output.ingredients.iter().enumerate() {
            let raw = ingredient.raw.clone().unwrap_or_default();
            if raw.is_empty() {
                continue;
            }

            // Run the full pipeline to get outputs for all steps
            let step_outputs = run_pipeline(&raw);

            let test_case = TestCase { raw, step_outputs };

            let filename = format!("{}--{}--{:02}.json", site, recipe_slug, idx + 1);
            let filepath = pipeline_dir.join(&filename);

            // UPSERT: only write if file doesn't already exist
            if !filepath.exists() {
                let json = serde_json::to_string_pretty(&test_case)?;
                fs::write(&filepath, json)?;
                total_cases += 1;
            }
        }
    }

    println!(
        "Generated {} new test cases in {}",
        total_cases,
        pipeline_dir.display()
    );

    Ok(())
}

/// Update all test fixtures to match current parser output.
///
/// Runs the pipeline on each test case's `raw` input and updates
/// the `step_outputs` field to match the actual output.
/// Also migrates legacy format (expected) to new format (step_outputs).
pub fn update_fixtures(fixtures_dir: Option<&Path>) -> Result<()> {
    let fixtures_dir = fixtures_dir
        .map(PathBuf::from)
        .unwrap_or_else(default_fixtures_dir);

    let mut updated = 0;
    let mut migrated = 0;
    let mut unchanged = 0;

    // Process curated, pipeline, and paprika directories
    for subdir in ["curated", "pipeline", "paprika"] {
        let dir = fixtures_dir.join(subdir);
        if !dir.exists() {
            continue;
        }

        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let content = fs::read_to_string(&path)?;

                // Try to parse as new format first
                let (raw, old_outputs) =
                    if let Ok(test_case) = serde_json::from_str::<TestCase>(&content) {
                        (test_case.raw, Some(test_case.step_outputs))
                    } else if let Ok(legacy) = serde_json::from_str::<LegacyTestCase>(&content) {
                        // Migrate from legacy format
                        migrated += 1;
                        (legacy.raw, None)
                    } else {
                        println!("Skipping invalid file: {}", path.display());
                        continue;
                    };

                // Run pipeline to get current outputs
                let new_outputs = run_pipeline(&raw);

                // Check if outputs changed
                let changed = old_outputs.as_ref() != Some(&new_outputs);

                if changed {
                    let test_case = TestCase {
                        raw,
                        step_outputs: new_outputs,
                    };
                    let json = serde_json::to_string_pretty(&test_case)?;
                    fs::write(&path, json)?;
                    updated += 1;
                    println!("Updated: {}", path.display());
                } else {
                    unchanged += 1;
                }
            }
        }
    }

    println!(
        "\nSummary: {} updated, {} migrated, {} unchanged",
        updated, migrated, unchanged
    );

    Ok(())
}

/// Find the latest pipeline run directory
fn find_latest_run(runs_dir: &Path) -> Result<(String, PathBuf)> {
    let mut runs: Vec<_> = fs::read_dir(runs_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .collect();

    runs.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

    let latest = runs.into_iter().next().context("No pipeline runs found")?;

    let run_id = latest.file_name().unwrap().to_string_lossy().into_owned();

    Ok((run_id, latest))
}

/// Extract site name from URL directory name
fn extract_site_from_url(url: &str) -> String {
    // URL format is: site-com_recipe-slug (e.g., "101cookbooks-com_coleslaw-recipe")
    // Extract the site part before the first underscore
    if let Some(site_part) = url.split('_').next() {
        // Remove the TLD suffix (e.g., "-com", "-co-uk")
        let site = site_part
            .trim_end_matches("-com")
            .trim_end_matches("-co-uk")
            .trim_end_matches("-org")
            .trim_end_matches("-net");
        if site.is_empty() {
            "unknown".to_string()
        } else {
            site.to_string()
        }
    } else {
        "unknown".to_string()
    }
}

/// Extract recipe slug from URL directory name
fn extract_recipe_slug(url: &str) -> String {
    // URL format is: site-com_recipe-slug (e.g., "101cookbooks-com_coleslaw-recipe")
    // Extract everything after the first underscore
    if let Some(idx) = url.find('_') {
        let slug = &url[idx + 1..];
        // Limit length and clean up
        let cleaned: String = slug
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .take(30)
            .collect();
        if cleaned.is_empty() {
            "recipe".to_string()
        } else {
            cleaned
        }
    } else {
        "recipe".to_string()
    }
}

/// ParseIngredientsOutput structure from pipeline
#[derive(Debug, Deserialize)]
struct ParseIngredientsOutput {
    ingredients: Vec<PipelineParsedIngredient>,
}

/// Parsed ingredient from pipeline output
#[derive(Debug, Deserialize)]
struct PipelineParsedIngredient {
    #[allow(dead_code)]
    item: String,
    #[allow(dead_code)]
    measurements: Vec<Measurement>,
    #[allow(dead_code)]
    note: Option<String>,
    raw: Option<String>,
}

/// Minimal Paprika recipe structure for ingredient extraction
#[derive(Debug, Deserialize)]
struct PaprikaRecipe {
    name: String,
    ingredients: Option<String>,
}

/// Generate test fixtures from a .paprikarecipes file.
///
/// Reads recipes from the Paprika archive and creates individual JSON
/// test case files in the paprika/ directory.
/// Uses UPSERT behavior: only adds new test cases, never deletes existing ones.
pub fn generate_from_paprika(paprika_file: &Path, fixtures_dir: Option<&Path>) -> Result<()> {
    let fixtures_dir = fixtures_dir
        .map(PathBuf::from)
        .unwrap_or_else(default_fixtures_dir);
    let paprika_dir = fixtures_dir.join("paprika");

    // Create directory if it doesn't exist (UPSERT: never delete existing)
    fs::create_dir_all(&paprika_dir)?;

    // Open the paprikarecipes archive
    let file = File::open(paprika_file)
        .with_context(|| format!("Failed to open file: {}", paprika_file.display()))?;

    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("Failed to read zip archive: {}", paprika_file.display()))?;

    println!("Reading recipes from: {}", paprika_file.display());

    let mut total_cases = 0;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let entry_name = entry.name().to_string();

        if !entry_name.ends_with(".paprikarecipe") {
            continue;
        }

        // Read the gzipped content
        let mut compressed_data = Vec::new();
        entry.read_to_end(&mut compressed_data)?;

        // Decompress with gzip
        let mut decoder = GzDecoder::new(&compressed_data[..]);
        let mut json_content = String::new();
        decoder
            .read_to_string(&mut json_content)
            .with_context(|| format!("Failed to decompress recipe: {}", entry_name))?;

        // Parse the recipe JSON
        let recipe: PaprikaRecipe = serde_json::from_str(&json_content)
            .with_context(|| format!("Failed to parse recipe JSON: {}", entry_name))?;

        // Skip recipes without ingredients
        let Some(ingredients_str) = recipe.ingredients.as_ref() else {
            continue;
        };

        if ingredients_str.trim().is_empty() {
            continue;
        }

        // Create recipe slug from name
        let recipe_slug = slugify_recipe_name(&recipe.name);

        // Parse each ingredient line and create test cases
        for (idx, line) in ingredients_str.lines().enumerate() {
            let raw = line.trim().to_string();
            if raw.is_empty() {
                continue;
            }

            // Run the full pipeline to get outputs for all steps
            let step_outputs = run_pipeline(&raw);

            let test_case = TestCase { raw, step_outputs };

            let filename = format!("paprika--{}--{:02}.json", recipe_slug, idx + 1);
            let filepath = paprika_dir.join(&filename);

            // UPSERT: only write if file doesn't already exist
            if !filepath.exists() {
                let json = serde_json::to_string_pretty(&test_case)?;
                fs::write(&filepath, json)?;
                total_cases += 1;
            }
        }
    }

    println!(
        "Generated {} new test cases in {}",
        total_cases,
        paprika_dir.display()
    );

    Ok(())
}

/// Convert a recipe name to a filesystem-safe slug
fn slugify_recipe_name(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();

    // Collapse multiple hyphens and trim
    let mut result = String::new();
    let mut prev_hyphen = false;
    for c in slug.chars().take(30) {
        if c == '-' {
            if !prev_hyphen && !result.is_empty() {
                result.push(c);
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }

    // Trim trailing hyphen
    while result.ends_with('-') {
        result.pop();
    }

    if result.is_empty() {
        "recipe".to_string()
    } else {
        result
    }
}
