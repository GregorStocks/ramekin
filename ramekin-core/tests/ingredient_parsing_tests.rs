//! Golden file tests for ingredient parsing.
//!
//! These tests verify that ingredient parsing produces expected results.
//! Test cases are individual JSON files in `fixtures/ingredient_parsing/`.
//!
//! Directory structure:
//! - `curated/` - Hand-picked test cases representing important scenarios
//! - `bulk/` - Auto-generated from pipeline runs for regression testing
//! - `paprika/` - Auto-generated from paprikarecipes file for regression testing

use glob::glob;
use ramekin_core::ingredient_parser::{parse_ingredient, Measurement, ParsedIngredient};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

/// A test case loaded from a JSON fixture file
#[derive(Debug, Deserialize)]
struct TestCase {
    /// Raw ingredient string to parse
    raw: String,
    /// Expected parsing result
    expected: ExpectedIngredient,
}

/// Expected ingredient parsing result (matches ParsedIngredient but without `raw` field)
#[derive(Debug, Deserialize, PartialEq)]
struct ExpectedIngredient {
    item: String,
    measurements: Vec<Measurement>,
    note: Option<String>,
}

impl From<ParsedIngredient> for ExpectedIngredient {
    fn from(parsed: ParsedIngredient) -> Self {
        Self {
            item: parsed.item,
            measurements: parsed.measurements,
            note: parsed.note,
        }
    }
}

/// Load all test cases from both curated and bulk directories
fn load_test_cases() -> Vec<(String, TestCase)> {
    let fixtures_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ingredient_parsing");

    let mut cases = Vec::new();

    // Load from curated, bulk, and paprika directories
    for subdir in ["curated", "bulk", "paprika"] {
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
        let actual = parse_ingredient(&case.raw);
        let actual_expected: ExpectedIngredient = actual.into();

        if actual_expected != case.expected {
            failures.push((name.clone(), case, actual_expected));
        }
    }

    if !failures.is_empty() {
        let mut msg = format!("\n{} of {} tests failed:\n", failures.len(), cases.len());

        for (name, case, actual) in &failures {
            msg.push_str(&format!("\n=== {} ===\n", name));
            msg.push_str(&format!("Input: {:?}\n", case.raw));
            msg.push_str(&format!("Expected: {:#?}\n", case.expected));
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
        let actual = parse_ingredient(&case.raw);
        let actual_expected: ExpectedIngredient = actual.into();

        if actual_expected != case.expected {
            failures.push((name.clone(), case, actual_expected));
        }
    }

    if !failures.is_empty() {
        let mut msg = format!(
            "\n{} of {} curated tests failed:\n",
            failures.len(),
            cases.len()
        );

        for (name, case, actual) in &failures {
            msg.push_str(&format!("\n=== {} ===\n", name));
            msg.push_str(&format!("Input: {:?}\n", case.raw));
            msg.push_str(&format!("Expected: {:#?}\n", case.expected));
            msg.push_str(&format!("Actual:   {:#?}\n", actual));
        }

        panic!("{}", msg);
    }

    println!("All {} curated tests passed!", cases.len());
}
