//! Golden file tests for ingredient parsing pipeline.
//!
//! These tests verify that the ingredient parsing pipeline produces expected results.
//! Test cases are JSON files in `fixtures/ingredient_parsing/`.
//!
//! Directory structure:
//! - `curated/` - Hand-picked test cases grouped by category (edge.json, unit_test.json, etc.)
//! - `pipeline/` - Auto-generated from pipeline runs, one file per recipe
//! - `paprika/` - Auto-generated from paprikarecipes file, one file per recipe
//!
//! Curated format (category files):
//! ```json
//! {
//!   "category": "edge",
//!   "test_cases": [
//!     { "name": "ingredient_alternative", "raw": "...", "expected": {...} }
//!   ]
//! }
//! ```
//!
//! Recipe format (pipeline/paprika):
//! ```json
//! {
//!   "source": "101cookbooks",
//!   "recipe_slug": "beet-caviar-recipe",
//!   "ingredients": [
//!     { "raw": "4 medium beets", "expected": {...} }
//!   ]
//! }
//! ```

use glob::glob;
use ramekin_core::ingredient_parser::{
    parse_ingredient, parse_ingredients, Measurement, ParsedIngredient,
};
use ramekin_core::metric_weights::{add_metric_weight_alternative, MetricConversionStats};
use ramekin_core::volume_to_weight::{add_volume_to_weight_alternative, VolumeConversionStats};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

/// A single test case with raw input and expected output
#[derive(Debug, Deserialize, Clone)]
struct TestCase {
    /// Raw ingredient string to parse
    raw: String,
    /// Expected output from parsing
    expected: Expected,
}

/// Recipe-based test file (for pipeline/paprika)
#[derive(Debug, Deserialize)]
struct RecipeTestFile {
    source: String,
    recipe_slug: String,
    ingredients: Vec<IngredientTestCase>,
}

/// A single ingredient test case within a recipe
#[derive(Debug, Deserialize)]
struct IngredientTestCase {
    raw: String,
    /// Expected output. None if this is a section header (filtered out).
    #[serde(default)]
    expected: Option<Expected>,
    /// True if this raw line is a section header (not an ingredient).
    #[serde(default)]
    is_section_header: bool,
}

/// Curated test file (category-based)
#[derive(Debug, Deserialize)]
struct CuratedTestFile {
    category: String,
    test_cases: Vec<CuratedTestCase>,
}

/// A single test case within a curated category file
#[derive(Debug, Deserialize)]
struct CuratedTestCase {
    name: String,
    raw: String,
    expected: Expected,
}

/// Expected output from parsing
#[derive(Debug, Deserialize, PartialEq, Clone)]
struct Expected {
    item: String,
    measurements: Vec<Measurement>,
    note: Option<String>,
    #[serde(default)]
    section: Option<String>,
}

impl From<ParsedIngredient> for Expected {
    fn from(parsed: ParsedIngredient) -> Self {
        Self {
            item: parsed.item,
            measurements: parsed.measurements,
            note: parsed.note,
            section: parsed.section,
        }
    }
}

/// Run the ingredient parsing pipeline on a raw ingredient string (single line).
/// Includes metric weight conversion (oz/lb → g) and volume-to-weight conversion.
fn run_pipeline(raw: &str) -> Expected {
    let parsed = parse_ingredient(raw);
    let mut weight_stats = MetricConversionStats::default();
    let mut volume_stats = VolumeConversionStats::default();
    let result = add_metric_weight_alternative(parsed, &mut weight_stats);
    let result = add_volume_to_weight_alternative(result, &mut volume_stats);
    Expected::from(result)
}

/// Run the ingredient parsing pipeline on a multi-line blob.
/// This includes section header detection, filtering, and applying sections to subsequent ingredients.
fn run_pipeline_batch(raw_lines: &[String]) -> Vec<Expected> {
    let blob = raw_lines.join("\n");
    let parsed = parse_ingredients(&blob);
    parsed
        .into_iter()
        .map(|ing| {
            let mut weight_stats = MetricConversionStats::default();
            let mut volume_stats = VolumeConversionStats::default();
            let result = add_metric_weight_alternative(ing, &mut weight_stats);
            let result = add_volume_to_weight_alternative(result, &mut volume_stats);
            Expected::from(result)
        })
        .collect()
}

/// A recipe batch test case (all ingredients processed together)
struct RecipeBatchTest {
    name: String,
    raw_lines: Vec<String>,
    expected: Vec<Expected>,
}

/// Load curated test cases (individual ingredient tests)
fn load_curated_test_cases() -> Vec<(String, TestCase)> {
    let fixtures_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ingredient_parsing/curated");

    let pattern = fixtures_dir.join("*.json");
    let pattern_str = pattern.to_string_lossy();

    let mut cases = Vec::new();
    for entry in glob(&pattern_str).expect("Failed to read glob pattern") {
        let path = entry.expect("Failed to read directory entry");
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));

        let file: CuratedTestFile = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e));

        for tc in &file.test_cases {
            let name = format!("curated/{}/{}", file.category, tc.name);
            cases.push((
                name,
                TestCase {
                    raw: tc.raw.clone(),
                    expected: tc.expected.clone(),
                },
            ));
        }
    }

    cases.sort_by(|a, b| a.0.cmp(&b.0));
    cases
}

/// Load recipe batch test cases (pipeline and paprika directories)
/// These are tested as batches to validate section header detection and application.
fn load_recipe_batch_tests() -> Vec<RecipeBatchTest> {
    let fixtures_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ingredient_parsing");

    let mut batches = Vec::new();

    for subdir in ["pipeline", "paprika"] {
        let pattern = fixtures_dir.join(subdir).join("*.json");
        let pattern_str = pattern.to_string_lossy();

        for entry in glob(&pattern_str).expect("Failed to read glob pattern") {
            let path = entry.expect("Failed to read directory entry");
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));

            let file: RecipeTestFile = serde_json::from_str(&content)
                .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e));

            let name = format!("{}/{}--{}", subdir, file.source, file.recipe_slug);
            // Collect all raw lines (including section headers) for batch processing
            let raw_lines: Vec<String> = file.ingredients.iter().map(|i| i.raw.clone()).collect();
            // Collect only non-section-header expected values (section headers have expected: None)
            let expected: Vec<Expected> = file
                .ingredients
                .iter()
                .filter_map(|i| i.expected.clone())
                .collect();

            batches.push(RecipeBatchTest {
                name,
                raw_lines,
                expected,
            });
        }
    }

    batches.sort_by(|a, b| a.name.cmp(&b.name));
    batches
}

#[test]
fn test_ingredient_parsing_golden_files() {
    // Test curated cases individually
    let curated_cases = load_curated_test_cases();
    let mut curated_failures = Vec::new();

    for (name, case) in &curated_cases {
        let actual = run_pipeline(&case.raw);

        if actual != case.expected {
            curated_failures.push((
                name.clone(),
                case.raw.clone(),
                case.expected.clone(),
                actual,
            ));
        }
    }

    // Test recipe files as batches (for section detection)
    let recipe_batches = load_recipe_batch_tests();
    let mut recipe_failures = Vec::new();

    for batch in &recipe_batches {
        let actual = run_pipeline_batch(&batch.raw_lines);

        if actual != batch.expected {
            recipe_failures.push((
                batch.name.clone(),
                batch.raw_lines.clone(),
                batch.expected.clone(),
                actual,
            ));
        }
    }

    let total_tests = curated_cases.len() + recipe_batches.len();
    let total_failures = curated_failures.len() + recipe_failures.len();

    if total_failures > 0 {
        let mut msg = format!(
            "\n{} failures across {} tests:\n",
            total_failures, total_tests
        );

        for (name, raw, expected, actual) in &curated_failures {
            msg.push_str(&format!("\n=== {} ===\n", name));
            msg.push_str(&format!("Input: {:?}\n", raw));
            msg.push_str(&format!("Expected: {:#?}\n", expected));
            msg.push_str(&format!("Actual:   {:#?}\n", actual));
        }

        for (name, raw_lines, expected, actual) in &recipe_failures {
            msg.push_str(&format!("\n=== {} ===\n", name));
            msg.push_str(&format!("Input lines ({}):\n", raw_lines.len()));
            for (i, line) in raw_lines.iter().enumerate() {
                msg.push_str(&format!("  [{}]: {:?}\n", i, line));
            }
            msg.push_str(&format!(
                "Expected ({} ingredients): {:#?}\n",
                expected.len(),
                expected
            ));
            msg.push_str(&format!(
                "Actual ({} ingredients):   {:#?}\n",
                actual.len(),
                actual
            ));
        }

        panic!("{}", msg);
    }

    println!(
        "All {} ingredient parsing tests passed! ({} curated, {} recipe batches)",
        total_tests,
        curated_cases.len(),
        recipe_batches.len()
    );
}

/// Run tests only from curated directory (for focused testing)
#[test]
fn test_ingredient_parsing_curated() {
    let cases = load_curated_test_cases();

    if cases.is_empty() {
        println!("No curated test fixtures found");
        return;
    }

    let mut failures = Vec::new();

    for (name, case) in &cases {
        let actual = run_pipeline(&case.raw);

        if actual != case.expected {
            failures.push((
                name.clone(),
                case.raw.clone(),
                case.expected.clone(),
                actual,
            ));
        }
    }

    if !failures.is_empty() {
        let mut msg = format!(
            "\n{} failures across {} curated tests:\n",
            failures.len(),
            cases.len()
        );

        for (name, raw, expected, actual) in &failures {
            msg.push_str(&format!("\n=== {} ===\n", name));
            msg.push_str(&format!("Input: {:?}\n", raw));
            msg.push_str(&format!("Expected: {:#?}\n", expected));
            msg.push_str(&format!("Actual:   {:#?}\n", actual));
        }

        panic!("{}", msg);
    }

    println!("All {} curated tests passed!", cases.len());
}
