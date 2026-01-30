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
//! Test format (new):
//! ```json
//! {
//!   "raw": "8 oz butter",
//!   "step_outputs": {
//!     "parse_ingredients": { "item": "butter", "measurements": [...], "note": null },
//!     "enrich_metric_weights": { "item": "butter", "measurements": [...], "note": null }
//!   }
//! }
//! ```

use glob::glob;
use ramekin_core::ingredient_parser::{parse_ingredient, Measurement, ParsedIngredient};
use ramekin_core::metric_weights::{add_metric_weight_alternative, EnrichmentStats};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// A test case loaded from a JSON fixture file (new format)
#[derive(Debug, Deserialize)]
struct TestCase {
    /// Raw ingredient string to parse
    raw: String,
    /// Expected outputs from each pipeline step
    step_outputs: HashMap<String, StepOutput>,
}

/// Legacy test case format (for migration support)
#[derive(Debug, Deserialize)]
struct LegacyTestCase {
    raw: String,
    expected: StepOutput,
}

/// Expected output from a single pipeline step
#[derive(Debug, Deserialize, PartialEq, Clone)]
struct StepOutput {
    item: String,
    measurements: Vec<Measurement>,
    note: Option<String>,
}

impl From<ParsedIngredient> for StepOutput {
    fn from(parsed: ParsedIngredient) -> Self {
        Self {
            item: parsed.item,
            measurements: parsed.measurements,
            note: parsed.note,
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

/// Loaded test case (unified format)
struct LoadedTestCase {
    raw: String,
    step_outputs: HashMap<String, StepOutput>,
}

/// Load a test case from JSON, supporting both old and new formats
fn load_test_case(content: &str) -> Result<LoadedTestCase, serde_json::Error> {
    // Try new format first
    if let Ok(case) = serde_json::from_str::<TestCase>(content) {
        return Ok(LoadedTestCase {
            raw: case.raw,
            step_outputs: case.step_outputs,
        });
    }

    // Fall back to legacy format
    let legacy: LegacyTestCase = serde_json::from_str(content)?;
    let mut step_outputs = HashMap::new();
    step_outputs.insert("parse_ingredients".to_string(), legacy.expected);
    Ok(LoadedTestCase {
        raw: legacy.raw,
        step_outputs,
    })
}

/// Run the full pipeline and return outputs for each step
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

/// Load all test cases from curated, pipeline, and paprika directories
fn load_test_cases() -> Vec<(String, LoadedTestCase)> {
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
            let case = load_test_case(&content)
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
        let actual_outputs = run_pipeline(&case.raw);

        // Check each step that has an expected output
        for (step_name, expected_output) in &case.step_outputs {
            if let Some(actual_output) = actual_outputs.get(step_name) {
                if actual_output != expected_output {
                    failures.push((
                        name.clone(),
                        step_name.clone(),
                        case.raw.clone(),
                        expected_output.clone(),
                        actual_output.clone(),
                    ));
                }
            }
        }
    }

    if !failures.is_empty() {
        let mut msg = format!(
            "\n{} step failures across {} tests:\n",
            failures.len(),
            cases.len()
        );

        for (name, step, raw, expected, actual) in &failures {
            msg.push_str(&format!("\n=== {} ({}) ===\n", name, step));
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
        let case = load_test_case(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e));
        cases.push((name, case));
    }

    if cases.is_empty() {
        println!("No curated test fixtures found");
        return;
    }

    let mut failures = Vec::new();

    for (name, case) in &cases {
        let actual_outputs = run_pipeline(&case.raw);

        for (step_name, expected_output) in &case.step_outputs {
            if let Some(actual_output) = actual_outputs.get(step_name) {
                if actual_output != expected_output {
                    failures.push((
                        name.clone(),
                        step_name.clone(),
                        case.raw.clone(),
                        expected_output.clone(),
                        actual_output.clone(),
                    ));
                }
            }
        }
    }

    if !failures.is_empty() {
        let mut msg = format!(
            "\n{} step failures across {} curated tests:\n",
            failures.len(),
            cases.len()
        );

        for (name, step, raw, expected, actual) in &failures {
            msg.push_str(&format!("\n=== {} ({}) ===\n", name, step));
            msg.push_str(&format!("Input: {:?}\n", raw));
            msg.push_str(&format!("Expected: {:#?}\n", expected));
            msg.push_str(&format!("Actual:   {:#?}\n", actual));
        }

        panic!("{}", msg);
    }

    println!("All {} curated tests passed!", cases.len());
}
