//! CLI commands for ingredient parsing test management.
//!
//! Provides commands to generate and update ingredient parsing test fixtures.

use anyhow::{Context, Result};
use ramekin_core::ingredient_parser::{parse_ingredient, Measurement};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// A test case for ingredient parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestCase {
    raw: String,
    expected: ExpectedIngredient,
}

/// Expected ingredient parsing result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ExpectedIngredient {
    item: String,
    measurements: Vec<Measurement>,
    note: Option<String>,
}

/// Default path to the fixtures directory
fn default_fixtures_dir() -> PathBuf {
    PathBuf::from("ramekin-core/tests/fixtures/ingredient_parsing")
}

/// Generate test fixtures from the latest pipeline run.
///
/// Reads parse_ingredients output from pipeline results and creates
/// individual JSON test case files in the bulk/ directory.
pub fn generate_from_pipeline(runs_dir: &Path, fixtures_dir: Option<&Path>) -> Result<()> {
    let fixtures_dir = fixtures_dir
        .map(PathBuf::from)
        .unwrap_or_else(default_fixtures_dir);
    let bulk_dir = fixtures_dir.join("bulk");

    // Find latest run
    let (run_id, run_dir) = find_latest_run(runs_dir)?;
    println!("Generating from run: {}", run_id);

    // Clear existing bulk tests
    if bulk_dir.exists() {
        fs::remove_dir_all(&bulk_dir)?;
    }
    fs::create_dir_all(&bulk_dir)?;

    let mut total_cases = 0;

    // Walk through all URL directories in the run
    for entry in fs::read_dir(&run_dir)? {
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

            let test_case = TestCase {
                raw: raw.clone(),
                expected: ExpectedIngredient {
                    item: ingredient.item.clone(),
                    measurements: ingredient.measurements.clone(),
                    note: ingredient.note.clone(),
                },
            };

            let filename = format!("{}--{}--{:02}.json", site, recipe_slug, idx + 1);
            let filepath = bulk_dir.join(&filename);

            let json = serde_json::to_string_pretty(&test_case)?;
            fs::write(&filepath, json)?;
            total_cases += 1;
        }
    }

    println!(
        "Generated {} test cases in {}",
        total_cases,
        bulk_dir.display()
    );

    Ok(())
}

/// Update all test fixtures to match current parser output.
///
/// Runs the parser on each test case's `raw` input and updates
/// the `expected` field to match the actual output.
pub fn update_fixtures(fixtures_dir: Option<&Path>) -> Result<()> {
    let fixtures_dir = fixtures_dir
        .map(PathBuf::from)
        .unwrap_or_else(default_fixtures_dir);

    let mut updated = 0;
    let mut unchanged = 0;

    // Process both curated and bulk directories
    for subdir in ["curated", "bulk"] {
        let dir = fixtures_dir.join(subdir);
        if !dir.exists() {
            continue;
        }

        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let content = fs::read_to_string(&path)?;
                let mut test_case: TestCase = serde_json::from_str(&content)?;

                // Run parser
                let actual = parse_ingredient(&test_case.raw);
                let actual_expected = ExpectedIngredient {
                    item: actual.item,
                    measurements: actual.measurements,
                    note: actual.note,
                };

                if actual_expected != test_case.expected {
                    test_case.expected = actual_expected;
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

    println!("\nSummary: {} updated, {} unchanged", updated, unchanged);

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
    // URL format is typically: https___www.site.com_path_to_recipe
    // Extract the domain part
    let parts: Vec<&str> = url.split("___").collect();
    if parts.len() >= 2 {
        let domain = parts[1].split('_').next().unwrap_or("unknown");
        domain
            .replace("www.", "")
            .split('.')
            .next()
            .unwrap_or("unknown")
            .to_string()
    } else {
        "unknown".to_string()
    }
}

/// Extract recipe slug from URL directory name
fn extract_recipe_slug(url: &str) -> String {
    // Take last path component, clean it up
    let parts: Vec<&str> = url.split('_').collect();
    if parts.len() >= 2 {
        let slug = parts.last().unwrap_or(&"recipe");
        // Limit length and clean up
        let cleaned = slug
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .take(30)
            .collect::<String>();
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
    ingredients: Vec<ParsedIngredient>,
}

/// Parsed ingredient from pipeline output
#[derive(Debug, Deserialize)]
struct ParsedIngredient {
    item: String,
    measurements: Vec<Measurement>,
    note: Option<String>,
    raw: Option<String>,
}
