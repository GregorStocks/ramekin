//! Categorizer scorecard against a real shopping-list corpus.
//!
//! The corpus is every distinct item ever added to the shopping list on the prod
//! server (including soft-deleted rows), extracted into `data/shopping-list-items.txt`
//! as `count<TAB>item`. Each distinct item is hand/LLM-labeled with the grocery-aisle
//! category a shopper would expect in `data/shopping-list-categories.tsv` as
//! `expected_category<TAB>item`.
//!
//! This test runs `ingredient_categorizer::categorize` over the labeled corpus and
//! reports the accuracy, the mismatches, and the "Other" rate (how much real usage the
//! categorizer fails to place). It is a regression guard via upper bounds on the
//! mismatch count and the Other count: improvements (fewer mismatches/Others) keep
//! passing, regressions fail. Tighten the bounds as the categorizer improves — see the
//! follow-up issue p2-expand-ingredient-categorizer-keywords, which uses this corpus.
//!
//! Run `make shopping-list-categorizer-test` to see the full report.

use ramekin_core::ingredient_categorizer::{categorize, CATEGORIES};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Upper bound on mismatches (categorizer disagrees with the expected label).
/// This is the current measured value; lower it as the categorizer improves.
const MAX_MISMATCHES: usize = 50;
/// Upper bound on items the categorizer returns "Other" for. Some of these are
/// genuinely non-food/uncategorizable items (household goods, etc.); others are
/// coverage gaps. Lower it as the categorizer improves.
const MAX_OTHER: usize = 29;

/// Project root (`ramekin-core/..`).
fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("ramekin-core has a parent")
        .to_path_buf()
}

/// Read a `<field><TAB><item>` data file into (field, item) pairs.
fn read_tsv(path: &Path) -> Vec<(String, String)> {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
    content
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (field, item) = line.split_once('\t').unwrap_or_else(|| {
                panic!(
                    "line is not tab-separated in {}: {:?}",
                    path.display(),
                    line
                )
            });
            (field.to_string(), item.to_string())
        })
        .collect()
}

#[test]
fn shopping_list_categorizer_scorecard() {
    let root = project_root();
    let labeled = read_tsv(&root.join("data/shopping-list-categories.tsv"));
    let corpus = read_tsv(&root.join("data/shopping-list-items.txt"));

    // The labeled fixture and the raw corpus must cover exactly the same items, so the
    // labels can't silently drift from the extracted corpus.
    let labeled_items: BTreeSet<&str> = labeled.iter().map(|(_, item)| item.as_str()).collect();
    let corpus_items: BTreeSet<&str> = corpus.iter().map(|(_, item)| item.as_str()).collect();
    assert_eq!(
        labeled_items, corpus_items,
        "data/shopping-list-categories.tsv and data/shopping-list-items.txt must label the same items"
    );

    // Every expected label must be a real category.
    for (expected, item) in &labeled {
        assert!(
            CATEGORIES.contains(&expected.as_str()),
            "{item:?} has invalid expected category {expected:?}"
        );
    }

    let total = labeled.len();
    let mut mismatches: Vec<(String, String, String)> = Vec::new();
    let mut other_items: Vec<String> = Vec::new();

    for (expected, item) in &labeled {
        let actual = categorize(item);
        if actual == "Other" {
            other_items.push(item.clone());
        }
        if actual != expected {
            mismatches.push((item.clone(), expected.clone(), actual.to_string()));
        }
    }

    let correct = total - mismatches.len();
    let accuracy = 100.0 * correct as f64 / total as f64;
    let other_rate = 100.0 * other_items.len() as f64 / total as f64;

    // Split the "Other" results into genuine coverage gaps (expected a real category
    // but got "Other") and items that are legitimately uncategorizable (expected Other).
    let coverage_gaps = mismatches
        .iter()
        .filter(|(_, _, actual)| actual == "Other")
        .count();
    let expected_other = other_items.len() - coverage_gaps;

    mismatches.sort();

    println!("\n=== Shopping-list categorizer scorecard ===");
    println!("items:      {total}");
    println!("correct:    {correct} ({accuracy:.1}%)");
    println!(
        "mismatches: {} ({:.1}%)",
        mismatches.len(),
        100.0 - accuracy
    );
    println!(
        "Other:      {} ({other_rate:.1}%) — {expected_other} expected Other, {coverage_gaps} coverage gaps",
        other_items.len()
    );

    if !mismatches.is_empty() {
        println!("\n--- mismatches (item | expected | got) ---");
        for (item, expected, actual) in &mismatches {
            println!("  {item}  |  {expected}  ->  {actual}");
        }
    }

    assert!(
        mismatches.len() <= MAX_MISMATCHES,
        "categorizer mismatches regressed: {} > {} (lower MAX_MISMATCHES once intentional)",
        mismatches.len(),
        MAX_MISMATCHES
    );
    assert!(
        other_items.len() <= MAX_OTHER,
        "categorizer 'Other' count regressed: {} > {} (lower MAX_OTHER once intentional)",
        other_items.len(),
        MAX_OTHER
    );
}
