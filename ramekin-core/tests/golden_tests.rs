//! Golden file tests for recipe extraction.
//!
//! These tests verify that extraction from HTML produces expected results.
//! Test cases are defined as JSON files in the `fixtures/` directory.

use ramekin_core::{extract_recipe_with_stats, ExtractionMethod};
use serde::Deserialize;
use std::fs;
use std::path::Path;

/// A test case loaded from a JSON fixture file
#[derive(Debug, Deserialize)]
struct TestCase {
    /// Path to the HTML fixture file (relative to project root)
    html_fixture_path: String,
    /// Source URL to use for extraction
    source_url: String,
    /// Expected extraction results
    expected: ExpectedRecipe,
}

/// Expected recipe extraction results
#[derive(Debug, Deserialize)]
struct ExpectedRecipe {
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    servings: Option<String>,
    #[serde(default)]
    method_used: Option<ExtractionMethod>,
    /// Raw ingredients as newline-separated string (current behavior)
    #[serde(default)]
    ingredients_raw: Option<String>,
    #[serde(default)]
    ingredients_contains: Vec<String>,
    #[serde(default)]
    ingredients_not_contains: Vec<String>,
    #[serde(default)]
    instructions_raw: Option<String>,
    #[serde(default)]
    instructions_contains: Vec<String>,
    #[serde(default)]
    instructions_not_contains: Vec<String>,
    #[serde(default)]
    footnotes: Vec<ExpectedFootnote>,
}

#[derive(Debug, Deserialize)]
struct ExpectedFootnote {
    marker: String,
    text_contains: String,
}

/// Get the project root directory
fn project_root() -> &'static Path {
    // ramekin-core/tests/golden_tests.rs -> go up to project root
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

/// Load all test cases from the fixtures directory
fn load_test_cases() -> Vec<(String, TestCase)> {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");

    let mut cases = Vec::new();

    for entry in fs::read_dir(&fixtures_dir).expect("Failed to read fixtures directory") {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();

        if path.extension().map(|e| e == "json").unwrap_or(false) {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
            let case: TestCase = serde_json::from_str(&content)
                .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e));
            cases.push((name, case));
        }
    }

    assert!(
        !cases.is_empty(),
        "No test fixtures found in {:?}",
        fixtures_dir
    );
    cases
}

#[test]
fn test_extraction_golden_files() {
    let cases = load_test_cases();

    for (name, case) in cases {
        println!("Testing: {}", name);

        // Load HTML fixture
        let html_path = project_root().join(&case.html_fixture_path);
        let html = fs::read_to_string(&html_path).unwrap_or_else(|e| {
            panic!("Failed to read HTML fixture {}: {}", html_path.display(), e)
        });

        // Run extraction
        let result = extract_recipe_with_stats(&html, &case.source_url)
            .unwrap_or_else(|e| panic!("Extraction failed for {}: {}", name, e));

        if let Some(expected_method) = case.expected.method_used {
            assert_eq!(
                result.method_used, expected_method,
                "Extraction method mismatch for {}",
                name
            );
        }

        // Verify title
        assert_eq!(
            result.raw_recipe.title, case.expected.title,
            "Title mismatch for {}",
            name
        );

        // Verify description if expected
        if let Some(expected_desc) = &case.expected.description {
            assert_eq!(
                result.raw_recipe.description.as_deref(),
                Some(expected_desc.as_str()),
                "Description mismatch for {}",
                name
            );
        }

        if let Some(expected_servings) = &case.expected.servings {
            assert_eq!(
                result.raw_recipe.servings.as_deref(),
                Some(expected_servings.as_str()),
                "Servings mismatch for {}",
                name
            );
        }

        if let Some(expected_ingredients) = &case.expected.ingredients_raw {
            assert_eq!(
                &result.raw_recipe.ingredients, expected_ingredients,
                "Ingredients mismatch for {}\n\nExpected:\n{}\n\nActual:\n{}",
                name, expected_ingredients, result.raw_recipe.ingredients
            );
        }

        for expected in &case.expected.ingredients_contains {
            assert!(
                result.raw_recipe.ingredients.contains(expected),
                "Ingredients for {} should contain {:?}\n\nActual:\n{}",
                name,
                expected,
                result.raw_recipe.ingredients
            );
        }

        for unexpected in &case.expected.ingredients_not_contains {
            assert!(
                !result.raw_recipe.ingredients.contains(unexpected),
                "Ingredients for {} should not contain {:?}\n\nActual:\n{}",
                name,
                unexpected,
                result.raw_recipe.ingredients
            );
        }

        if let Some(expected_instructions) = &case.expected.instructions_raw {
            assert_eq!(
                &result.raw_recipe.instructions, expected_instructions,
                "Instructions mismatch for {}\n\nExpected:\n{}\n\nActual:\n{}",
                name, expected_instructions, result.raw_recipe.instructions
            );
        }

        for expected in &case.expected.instructions_contains {
            assert!(
                result.raw_recipe.instructions.contains(expected),
                "Instructions for {} should contain {:?}\n\nActual:\n{}",
                name,
                expected,
                result.raw_recipe.instructions
            );
        }

        for unexpected in &case.expected.instructions_not_contains {
            assert!(
                !result.raw_recipe.instructions.contains(unexpected),
                "Instructions for {} should not contain {:?}\n\nActual:\n{}",
                name,
                unexpected,
                result.raw_recipe.instructions
            );
        }

        if !case.expected.footnotes.is_empty() {
            let actual_footnotes = result
                .raw_recipe
                .footnotes
                .as_ref()
                .unwrap_or_else(|| panic!("Expected footnotes for {}", name));
            assert_eq!(
                actual_footnotes.len(),
                case.expected.footnotes.len(),
                "Footnote count mismatch for {}",
                name
            );

            for (actual, expected) in actual_footnotes.iter().zip(&case.expected.footnotes) {
                assert_eq!(
                    actual.0, expected.marker,
                    "Footnote marker mismatch for {}",
                    name
                );
                assert!(
                    actual.1.contains(&expected.text_contains),
                    "Footnote for {} should contain {:?}\n\nActual:\n{}",
                    name,
                    expected.text_contains,
                    actual.1
                );
            }
        }
    }
}
