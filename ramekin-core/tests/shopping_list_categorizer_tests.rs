//! Categorizer scorecard against a real shopping-list corpus.
//!
//! The corpus is every distinct item ever added to the shopping list on the prod
//! server (including soft-deleted rows), extracted into `data/shopping-list-items.txt`
//! as `count<TAB>item`. Each distinct item is hand/LLM-labeled with the grocery-aisle
//! category a shopper would expect in `data/shopping-list-categories.tsv` as
//! `expected_category<TAB>item`.
//!
//! This test runs `ingredient_categorizer::categorize` over the labeled corpus and
//! reports accuracy, the mismatches, and the "Other" rate (how much real usage the
//! categorizer fails to place). It reports two ways: per distinct item, and weighted
//! by usage count — so a regression on frequently-added items can't hide behind
//! improvements on one-off items. It is a regression guard via upper bounds on the
//! distinct and weighted mismatch/Other counts: improvements keep passing, regressions
//! fail. Tighten the bounds as the categorizer improves — see the follow-up issue
//! p2-expand-ingredient-categorizer-keywords, which uses this corpus.
//!
//! Run `make shopping-list-categorizer-test` to see the full report.

use ramekin_core::ingredient_categorizer::{categorize, CATEGORIES};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

/// Upper bound on distinct-item mismatches (categorizer disagrees with the label).
/// These are the current measured values; lower them as the categorizer improves.
const MAX_MISMATCHES: usize = 50;
/// Upper bound on distinct items the categorizer returns "Other" for. Some are
/// genuinely non-food/uncategorizable (household goods, etc.); others are coverage gaps.
const MAX_OTHER: usize = 29;
/// Upper bound on usage-weighted mismatches (mismatches × times the item was added).
const MAX_WEIGHTED_MISMATCHES: u64 = 68;
/// Upper bound on usage-weighted "Other" results (Other × times the item was added).
const MAX_WEIGHTED_OTHER: u64 = 39;

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

    // `corpus` is `count<TAB>item`; build item -> usage count.
    let counts: HashMap<&str, u64> = corpus
        .iter()
        .map(|(count, item)| {
            let n = count
                .parse::<u64>()
                .unwrap_or_else(|_| panic!("non-numeric count for {item:?}: {count:?}"));
            (item.as_str(), n)
        })
        .collect();

    // The labeled fixture and the raw corpus must cover exactly the same items, so the
    // labels can't silently drift from the extracted corpus.
    let labeled_items: BTreeSet<&str> = labeled.iter().map(|(_, item)| item.as_str()).collect();
    let corpus_items: BTreeSet<&str> = counts.keys().copied().collect();
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
    let weighted_total: u64 = counts.values().sum();
    let mut mismatches: Vec<(String, String, String)> = Vec::new();
    let mut other_items: Vec<String> = Vec::new();
    let mut weighted_mismatches: u64 = 0;
    let mut weighted_other: u64 = 0;

    for (expected, item) in &labeled {
        let actual = categorize(item);
        let weight = counts[item.as_str()];
        if actual == "Other" {
            other_items.push(item.clone());
            weighted_other += weight;
        }
        if actual != expected {
            mismatches.push((item.clone(), expected.clone(), actual.to_string()));
            weighted_mismatches += weight;
        }
    }

    let correct = total - mismatches.len();
    let accuracy = 100.0 * correct as f64 / total as f64;
    let other_rate = 100.0 * other_items.len() as f64 / total as f64;
    let weighted_accuracy =
        100.0 * (weighted_total - weighted_mismatches) as f64 / weighted_total as f64;
    let weighted_other_rate = 100.0 * weighted_other as f64 / weighted_total as f64;

    // Split the "Other" results into genuine coverage gaps (expected a real category
    // but got "Other") and items that are legitimately uncategorizable (expected Other).
    let coverage_gaps = mismatches
        .iter()
        .filter(|(_, _, actual)| actual == "Other")
        .count();
    let expected_other = other_items.len() - coverage_gaps;

    mismatches.sort();

    println!("\n=== Shopping-list categorizer scorecard ===");
    println!("distinct items: {total}");
    println!("  correct:    {correct} ({accuracy:.1}%)");
    println!(
        "  mismatches: {} ({:.1}%)",
        mismatches.len(),
        100.0 - accuracy
    );
    println!(
        "  Other:      {} ({other_rate:.1}%) — {expected_other} expected Other, {coverage_gaps} coverage gaps",
        other_items.len()
    );
    println!("usage-weighted ({weighted_total} adds):");
    println!(
        "  correct:    {} ({weighted_accuracy:.1}%)",
        weighted_total - weighted_mismatches
    );
    println!(
        "  mismatches: {weighted_mismatches} ({:.1}%)",
        100.0 - weighted_accuracy
    );
    println!("  Other:      {weighted_other} ({weighted_other_rate:.1}%)");

    if !mismatches.is_empty() {
        println!("\n--- mismatches (count | item | expected | got) ---");
        for (item, expected, actual) in &mismatches {
            println!(
                "  {:>3}  {item}  |  {expected}  ->  {actual}",
                counts[item.as_str()]
            );
        }
    }

    assert!(
        mismatches.len() <= MAX_MISMATCHES,
        "distinct mismatches regressed: {} > {} (lower MAX_MISMATCHES once intentional)",
        mismatches.len(),
        MAX_MISMATCHES
    );
    assert!(
        other_items.len() <= MAX_OTHER,
        "distinct 'Other' count regressed: {} > {} (lower MAX_OTHER once intentional)",
        other_items.len(),
        MAX_OTHER
    );
    assert!(
        weighted_mismatches <= MAX_WEIGHTED_MISMATCHES,
        "weighted mismatches regressed: {weighted_mismatches} > {MAX_WEIGHTED_MISMATCHES} (lower once intentional)"
    );
    assert!(
        weighted_other <= MAX_WEIGHTED_OTHER,
        "weighted 'Other' regressed: {weighted_other} > {MAX_WEIGHTED_OTHER} (lower once intentional)"
    );
}
