//! Golden file tests for ingredient parsing pipeline.
//!
//! These tests verify that the ingredient parsing pipeline produces expected results.
//! Test cases are individual JSON files in `fixtures/ingredient_parsing/`.
//!
//! Directory structure:
//! - `curated/` - Hand-picked test cases representing important scenarios
//! - `pipeline/` - Auto-generated from pipeline runs for regression testing
//! - `paprika/` - Auto-generated from paprikarecipes file for regression testing
//!
//! Test format:
//! ```json
//! {
//!   "raw": "8 oz butter",
//!   "expected": { "item": "butter", "measurements": [...], "note": null }
//! }
//! ```

use glob::glob;
use ramekin_core::ingredient_parser::{parse_ingredient, Measurement, ParsedIngredient};
use ramekin_core::metric_weights::{add_metric_weight_alternative, MetricConversionStats};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

/// A test case loaded from a JSON fixture file
#[derive(Debug, Deserialize)]
struct TestCase {
    /// Raw ingredient string to parse
    raw: String,
    /// Expected output from parsing
    expected: Expected,
}

/// Expected output from parsing
#[derive(Debug, Deserialize, PartialEq, Clone)]
struct Expected {
    item: String,
    measurements: Vec<Measurement>,
    note: Option<String>,
}

impl From<ParsedIngredient> for Expected {
    fn from(parsed: ParsedIngredient) -> Self {
        Self {
            item: parsed.item,
            measurements: parsed.measurements,
            note: parsed.note,
        }
    }
}

/// Run the ingredient parsing pipeline on a raw ingredient string.
/// Includes metric weight conversion (oz → g).
fn run_pipeline(raw: &str) -> Expected {
    let parsed = parse_ingredient(raw);
    let mut stats = MetricConversionStats::default();
    let result = add_metric_weight_alternative(parsed, &mut stats);
    Expected::from(result)
}

/// Load all test cases from curated, pipeline, and paprika directories
fn load_test_cases() -> Vec<(String, TestCase)> {
    let fixtures_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ingredient_parsing");

    let mut cases = Vec::new();

    // Load from curated, pipeline, and paprika directories
    for subdir in ["curated", "pipeline", "paprika"] {
        let pattern = fixtures_dir.join(subdir).join("*.json");
        let pattern_str = pattern.to_string_lossy();

        for entry in glob(&pattern_str).expect("Failed to read glob pattern") {
            let path = entry.expect("Failed to read directory entry");
            let name = format!("{}/{}", subdir, path.file_stem().unwrap().to_string_lossy());
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
            let case: TestCase = serde_json::from_str(&content)
                .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e));
            cases.push((name, case));
        }
    }

    // Sort by name for deterministic ordering
    cases.sort_by(|a, b| a.0.cmp(&b.0));

    cases
}

#[test]
fn test_ingredient_parsing_golden_files() {
    let cases = load_test_cases();

    if cases.is_empty() {
        println!("No test fixtures found - this is OK for initial setup");
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
            "\n{} failures across {} tests:\n",
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

    println!("All {} ingredient parsing tests passed!", cases.len());
}

/// Run tests only from curated directory (for focused testing)
#[test]
fn test_ingredient_parsing_curated() {
    let fixtures_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ingredient_parsing/curated");

    let pattern = fixtures_dir.join("*.json");
    let pattern_str = pattern.to_string_lossy();

    let mut cases = Vec::new();
    for entry in glob(&pattern_str).expect("Failed to read glob pattern") {
        let path = entry.expect("Failed to read directory entry");
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
        let case: TestCase = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e));
        cases.push((name, case));
    }

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
