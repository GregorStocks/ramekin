//! CLI commands for ingredient parsing test management.
//!
//! Provides commands to generate and update ingredient parsing test fixtures.

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use ramekin_core::ingredient_categorizer;
use ramekin_core::ingredient_parser::{
    detect_section_header, parse_ingredient, parse_ingredients, should_ignore_line, Measurement,
    ParsedIngredient,
};
use ramekin_core::metric_weights::{add_metric_weight_alternative, MetricConversionStats};
use ramekin_core::volume_to_weight::{
    add_volume_to_weight_alternative, apply_ingredient_rewrites, VolumeConversionStats,
};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

/// A test case for ingredient parsing (legacy format, used for curated migration)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestCase {
    raw: String,
    expected: Expected,
}

/// Recipe-based test file (for pipeline/paprika)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecipeTestFile {
    source: String,
    recipe_slug: String,
    ingredients: Vec<IngredientTestCase>,
}

/// A single ingredient test case within a recipe
#[derive(Debug, Clone, Serialize, Deserialize)]
struct IngredientTestCase {
    raw: String,
    /// Expected output. None if this is a section header (filtered out).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expected: Option<Expected>,
    /// True if this raw line is a section header (not an ingredient).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    is_section_header: bool,
    /// True if this entry is a continuation of an "each" expansion from the previous entry's raw line.
    /// When building the ingredient blob for batch processing, expanded entries should be skipped.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    expanded: bool,
}

/// Expected output from parsing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Expected {
    item: String,
    measurements: Vec<Measurement>,
    note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    section: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    category: Option<String>,
}

impl From<ParsedIngredient> for Expected {
    fn from(ing: ParsedIngredient) -> Self {
        let category = ingredient_categorizer::categorize(&ing.item).to_string();
        Self {
            item: ing.item,
            measurements: ing.measurements,
            note: ing.note,
            section: ing.section,
            category: Some(category),
        }
    }
}

/// Default path to the fixtures directory
fn default_fixtures_dir() -> PathBuf {
    PathBuf::from("ramekin-core/tests/fixtures/ingredient_parsing")
}

/// Run the ingredient parsing pipeline on a raw ingredient string (single line).
/// Includes metric weight conversion (oz/lb → g) and volume-to-weight conversion.
fn run_pipeline(raw: &str) -> Expected {
    let parsed = parse_ingredient(raw);
    let mut weight_stats = MetricConversionStats::default();
    let mut volume_stats = VolumeConversionStats::default();
    let result = apply_ingredient_rewrites(parsed);
    let result = add_metric_weight_alternative(result, &mut weight_stats);
    let result = add_volume_to_weight_alternative(result, &mut volume_stats);
    let result = result.normalize_amounts();
    Expected::from(result)
}

/// Result of batch processing ingredients, including the raw line for each output.
struct BatchResult {
    raw: String,
    expected: Expected,
}

/// Run the ingredient parsing pipeline on a multi-line blob.
/// This includes section header detection, filtering, and applying sections to subsequent ingredients.
/// Returns both the raw line and expected output for each ingredient (section headers are filtered out).
fn run_pipeline_batch(raw_lines: &[String]) -> Vec<BatchResult> {
    let blob = raw_lines.join("\n");
    let parsed = parse_ingredients(&blob);
    parsed
        .into_iter()
        .map(|ing| {
            let raw = ing.raw.clone().unwrap_or_default();
            let mut weight_stats = MetricConversionStats::default();
            let mut volume_stats = VolumeConversionStats::default();
            let result = apply_ingredient_rewrites(ing);
            let result = add_metric_weight_alternative(result, &mut weight_stats);
            let result = add_volume_to_weight_alternative(result, &mut volume_stats);
            let result = result.normalize_amounts();
            BatchResult {
                raw,
                expected: Expected::from(result),
            }
        })
        .collect()
}

/// Generate test fixtures from the latest pipeline run.
///
/// Reads parse_ingredients output from pipeline results and creates
/// one JSON file per recipe in the pipeline/ directory.
/// Uses UPSERT behavior: only adds new recipes, never deletes existing ones.
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

    let mut total_recipes = 0;
    let mut total_ingredients = 0;

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

        // Collect all ingredients for this recipe
        let mut ingredients = Vec::new();
        for ingredient in &output.ingredients {
            let raw = ingredient.raw.clone().unwrap_or_default();
            if raw.is_empty() {
                continue;
            }

            let expected = run_pipeline(&raw);
            ingredients.push(IngredientTestCase {
                raw,
                expected: Some(expected),
                is_section_header: false,
                expanded: false,
            });
        }

        if ingredients.is_empty() {
            continue;
        }

        let recipe_file = RecipeTestFile {
            source: site.clone(),
            recipe_slug: recipe_slug.clone(),
            ingredients,
        };

        let filename = format!("{}--{}.json", site, recipe_slug);
        let filepath = pipeline_dir.join(&filename);

        // UPSERT: only write if file doesn't already exist
        if !filepath.exists() {
            total_ingredients += recipe_file.ingredients.len();
            let json = serde_json::to_string_pretty(&recipe_file)? + "\n";
            fs::write(&filepath, json)?;
            total_recipes += 1;
        }
    }

    println!(
        "Generated {} new recipes ({} ingredients) in {}",
        total_recipes,
        total_ingredients,
        pipeline_dir.display()
    );

    Ok(())
}

/// Update all test fixtures to match current parser output.
///
/// Runs the pipeline on each test case's `raw` input and updates
/// the `expected` field to match the actual output.
/// Handles both new formats (recipe and curated) and legacy single-case format.
pub fn update_fixtures(fixtures_dir: Option<&Path>) -> Result<()> {
    let fixtures_dir = fixtures_dir
        .map(PathBuf::from)
        .unwrap_or_else(default_fixtures_dir);

    let mut updated = 0;
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

            if !path.extension().map(|e| e == "json").unwrap_or(false) {
                continue;
            }

            let content = fs::read_to_string(&path)?;
            let json: serde_json::Value = serde_json::from_str(&content)?;

            if json.get("ingredients").is_some() {
                // Recipe format (pipeline/paprika) - process as batch for section detection
                let recipe: RecipeTestFile = serde_json::from_value(json)?;

                // Collect raw lines, skipping expanded entries (continuations of "each" expansion)
                let raw_lines: Vec<String> = recipe
                    .ingredients
                    .iter()
                    .filter(|i| !i.expanded)
                    .map(|i| i.raw.clone())
                    .collect();
                let batch_results = run_pipeline_batch(&raw_lines);

                // Build new ingredients list, preserving section headers as markers
                // Skip lines that should be ignored (scraper artifacts)
                let mut new_ingredients = Vec::new();
                let mut batch_iter = batch_results.into_iter().peekable();

                for raw in &raw_lines {
                    // Skip lines that should be ignored (scraper artifacts like "Gather Your Ingredients")
                    if should_ignore_line(raw) {
                        continue;
                    }
                    if detect_section_header(raw).is_some() {
                        // This is a section header - mark it as such
                        new_ingredients.push(IngredientTestCase {
                            raw: raw.clone(),
                            expected: None,
                            is_section_header: true,
                            expanded: false,
                        });
                    } else {
                        // Regular ingredient - consume one batch result for this raw line,
                        // then consume additional "each" expansion results (same raw, different item).
                        // This avoids incorrectly treating duplicate raw lines (e.g., butter used
                        // in two recipe steps) as "each" expansions.
                        if let Some(result) = batch_iter.peek() {
                            if result.raw == *raw {
                                let first_result = batch_iter.next().unwrap();
                                let first_item = first_result.expected.item.clone();
                                new_ingredients.push(IngredientTestCase {
                                    raw: first_result.raw,
                                    expected: Some(first_result.expected),
                                    is_section_header: false,
                                    expanded: false,
                                });
                                // Consume additional "each" expansion results (same raw line, different item)
                                while let Some(next) = batch_iter.peek() {
                                    if next.raw == *raw && next.expected.item != first_item {
                                        let next = batch_iter.next().unwrap();
                                        new_ingredients.push(IngredientTestCase {
                                            raw: next.raw,
                                            expected: Some(next.expected),
                                            is_section_header: false,
                                            expanded: true,
                                        });
                                    } else {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                // Check if anything changed
                let old_ingredients = &recipe.ingredients;
                let len_changed = old_ingredients.len() != new_ingredients.len();
                let content_changed =
                    old_ingredients
                        .iter()
                        .zip(new_ingredients.iter())
                        .any(|(old, new)| {
                            old.raw != new.raw
                                || old.expected != new.expected
                                || old.is_section_header != new.is_section_header
                        });
                let changed = len_changed || content_changed;

                if changed {
                    let new_recipe = RecipeTestFile {
                        source: recipe.source.clone(),
                        recipe_slug: recipe.recipe_slug.clone(),
                        ingredients: new_ingredients,
                    };

                    let json = serde_json::to_string_pretty(&new_recipe)? + "\n";
                    fs::write(&path, json)?;
                    updated += 1;
                } else {
                    unchanged += 1;
                }
            } else if json.get("test_cases").is_some() {
                // Curated format
                let mut curated: CuratedTestFile = serde_json::from_value(json)?;
                let mut file_changed = false;

                for tc in &mut curated.test_cases {
                    let new_expected = run_pipeline(&tc.raw);
                    if tc.expected != new_expected {
                        tc.expected = new_expected;
                        file_changed = true;
                    }
                }

                if file_changed {
                    let json = serde_json::to_string_pretty(&curated)? + "\n";
                    fs::write(&path, json)?;
                    updated += 1;
                } else {
                    unchanged += 1;
                }
            } else {
                println!("Skipping unknown format: {}", path.display());
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
    if let Some((_, slug)) = url.split_once('_') {
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
/// Reads recipes from the Paprika archive and creates one JSON file per recipe
/// in the paprika/ directory.
/// Uses UPSERT behavior: only adds new recipes, never deletes existing ones.
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

    let mut total_recipes = 0;
    let mut total_ingredients = 0;

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

        // Collect all ingredients for this recipe
        let mut ingredients = Vec::new();
        for line in ingredients_str.lines() {
            let raw = line.trim().to_string();
            if raw.is_empty() {
                continue;
            }

            let expected = run_pipeline(&raw);
            ingredients.push(IngredientTestCase {
                raw,
                expected: Some(expected),
                is_section_header: false,
                expanded: false,
            });
        }

        if ingredients.is_empty() {
            continue;
        }

        let recipe_file = RecipeTestFile {
            source: "paprika".to_string(),
            recipe_slug: recipe_slug.clone(),
            ingredients,
        };

        let filename = format!("paprika--{}.json", recipe_slug);
        let filepath = paprika_dir.join(&filename);

        // UPSERT: only write if file doesn't already exist
        if !filepath.exists() {
            total_ingredients += recipe_file.ingredients.len();
            let json = serde_json::to_string_pretty(&recipe_file)? + "\n";
            fs::write(&filepath, json)?;
            total_recipes += 1;
        }
    }

    println!(
        "Generated {} new recipes ({} ingredients) in {}",
        total_recipes,
        total_ingredients,
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

/// Curated test file format (category-based)
#[derive(Debug, Serialize, Deserialize)]
struct CuratedTestFile {
    category: String,
    test_cases: Vec<CuratedTestCase>,
}

/// A single test case within a curated category file
#[derive(Debug, Serialize, Deserialize)]
struct CuratedTestCase {
    name: String,
    raw: String,
    expected: Expected,
}

/// Migrate curated fixtures from individual files to category files.
///
/// Reads all individual test case files from curated/, groups them by category,
/// and writes consolidated category files.
pub fn migrate_curated(fixtures_dir: Option<&Path>) -> Result<()> {
    let fixtures_dir = fixtures_dir
        .map(PathBuf::from)
        .unwrap_or_else(default_fixtures_dir);
    let curated_dir = fixtures_dir.join("curated");

    if !curated_dir.exists() {
        println!("No curated directory found at {}", curated_dir.display());
        return Ok(());
    }

    // Collect all test cases grouped by category
    let mut categories: std::collections::HashMap<String, Vec<CuratedTestCase>> =
        std::collections::HashMap::new();
    let mut files_to_delete = Vec::new();

    for entry in fs::read_dir(&curated_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.extension().map(|e| e == "json").unwrap_or(false) {
            continue;
        }

        let filename = path.file_stem().unwrap().to_string_lossy();

        // Parse filename: category--name--NN.json
        let parts: Vec<&str> = filename.split("--").collect();
        if parts.len() < 2 {
            println!("Skipping file with unexpected format: {}", filename);
            continue;
        }

        let category = parts[0].to_string();
        // Combine remaining parts as the test name (excluding the number suffix)
        let name = if parts.len() >= 3 {
            // Format: category--subcategory--NN
            parts[1].to_string()
        } else {
            // Format: category--NN (shouldn't happen but handle it)
            "test".to_string()
        };

        // Read and parse the test case
        let content = fs::read_to_string(&path)?;
        let test_case: TestCase = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        categories
            .entry(category)
            .or_default()
            .push(CuratedTestCase {
                name,
                raw: test_case.raw,
                expected: test_case.expected,
            });

        files_to_delete.push(path);
    }

    if categories.is_empty() {
        println!("No curated test cases found to migrate");
        return Ok(());
    }

    // Write consolidated category files
    for (category, mut test_cases) in categories {
        // Sort test cases by name for deterministic ordering
        test_cases.sort_by(|a, b| a.name.cmp(&b.name));

        let file = CuratedTestFile {
            category: category.clone(),
            test_cases,
        };

        let filename = format!("{}.json", category);
        let filepath = curated_dir.join(&filename);

        let json = serde_json::to_string_pretty(&file)? + "\n";
        fs::write(&filepath, &json)?;
        println!(
            "Created {} with {} test cases",
            filename,
            file.test_cases.len()
        );
    }

    // Delete old individual files
    for path in &files_to_delete {
        fs::remove_file(path)?;
    }
    println!("\nDeleted {} old individual files", files_to_delete.len());

    Ok(())
}
