//! Categorizer scorecard against a real shopping-list corpus.
//!
//! The corpus is every distinct item typed into the shopping list on the prod
//! server (including soft-deleted rows), extracted into
//! `data/shopping-list-categories.json` as `{item, count, category}` entries, where
//! `count` is how many times the item was added and `category` is the hand/LLM-labeled
//! grocery-aisle category a shopper would expect. Items added from a recipe are left
//! out: they are copies of the recipe's parse when they were added, and recipe lines
//! are scored by the ingredient fixtures, which follow the parser.
//!
//! This test runs `ingredient_categorizer::categorize` over the labeled corpus and
//! reports accuracy, the mismatches, and the "Other" rate (how much real usage the
//! categorizer fails to place). It reports two ways: per distinct item, and weighted
//! by usage count — so a regression on frequently-added items can't hide behind
//! improvements on one-off items. It is a regression guard via upper bounds on the
//! distinct and weighted mismatch/Other counts: improvements keep passing, regressions
//! fail. Tighten the bounds as the categorizer improves.
//!
//! Run `make shopping-list-categorizer-test` to see the full report.

use ramekin_core::catalog::Learned;
use ramekin_core::ingredient_categorizer::{catalog_category_with, categorize, CATEGORIES};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Upper bound on distinct-item mismatches (categorizer disagrees with the label).
/// These are the current measured values; lower them as the categorizer improves.
/// Most of what's left is household goods and items the keyword rules don't know
/// (`issues/*shopping-hand-typed-misses*`), plus "Banning soda" (typo for baking
/// soda).
const MAX_MISMATCHES: usize = 16;
/// Upper bound on distinct items the categorizer returns "Other" for. Five are
/// labeled Other ("Cold salad", "Dip", "Go humans", "Hello", "Sandwich stuff").
const MAX_OTHER: usize = 14;
/// Upper bound on usage-weighted mismatches (mismatches × times the item was added).
const MAX_WEIGHTED_MISMATCHES: u64 = 17;
/// Upper bound on usage-weighted "Other" results (Other × times the item was added).
const MAX_WEIGHTED_OTHER: u64 = 14;
/// Upper bounds on distinct mismatches and "Other" results from the catalog alone,
/// without the keyword rules. Deleting the keyword rules
/// (`issues/*catalog-delete-keyword-categorizer*`) needs these at the bounds above.
const MAX_CATALOG_ONLY_MISMATCHES: usize = 31;
const MAX_CATALOG_ONLY_OTHER: usize = 36;

/// One labeled corpus entry from `data/shopping-list-categories.json`.
#[derive(Deserialize)]
struct CorpusEntry {
    /// The shopping-list item string exactly as the user (or recipe import) entered it.
    item: String,
    /// How many times the item was added on prod (including soft-deleted rows).
    count: u64,
    /// The expected grocery-aisle category.
    category: String,
}

fn load_corpus() -> Vec<CorpusEntry> {
    // ramekin-core/tests -> project root
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("ramekin-core has a parent")
        .join("data/shopping-list-categories.json");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("failed to parse {}: {}", path.display(), e))
}

#[test]
fn shopping_list_categorizer_scorecard() {
    let corpus = load_corpus();

    // Entries must be distinct items with valid expected categories.
    let mut seen = BTreeSet::new();
    for entry in &corpus {
        assert!(
            seen.insert(entry.item.as_str()),
            "duplicate corpus item {:?}",
            entry.item
        );
        assert!(
            CATEGORIES.contains(&entry.category.as_str()),
            "{:?} has invalid expected category {:?}",
            entry.item,
            entry.category
        );
    }

    let total = corpus.len();
    let weighted_total: u64 = corpus.iter().map(|e| e.count).sum();
    let mut mismatches: Vec<&CorpusEntry> = Vec::new();
    let mut other = 0usize;
    let mut weighted_mismatches: u64 = 0;
    let mut weighted_other: u64 = 0;

    for entry in &corpus {
        let actual = categorize(&entry.item);
        if actual == "Other" {
            other += 1;
            weighted_other += entry.count;
        }
        if actual != entry.category {
            mismatches.push(entry);
            weighted_mismatches += entry.count;
        }
    }

    let correct = total - mismatches.len();
    let accuracy = 100.0 * correct as f64 / total as f64;
    let other_rate = 100.0 * other as f64 / total as f64;
    let weighted_accuracy =
        100.0 * (weighted_total - weighted_mismatches) as f64 / weighted_total as f64;
    let weighted_other_rate = 100.0 * weighted_other as f64 / weighted_total as f64;

    // Split the "Other" results into genuine coverage gaps (expected a real category
    // but got "Other") and items that are legitimately uncategorizable (expected Other).
    let coverage_gaps = mismatches
        .iter()
        .filter(|e| categorize(&e.item) == "Other")
        .count();
    let expected_other = other - coverage_gaps;

    mismatches.sort_by(|a, b| a.item.cmp(&b.item));

    println!("\n=== Shopping-list categorizer scorecard ===");
    println!("distinct items: {total}");
    println!("  correct:    {correct} ({accuracy:.1}%)");
    println!(
        "  mismatches: {} ({:.1}%)",
        mismatches.len(),
        100.0 - accuracy
    );
    println!(
        "  Other:      {other} ({other_rate:.1}%) — {expected_other} expected Other, {coverage_gaps} coverage gaps"
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
        for entry in &mismatches {
            println!(
                "  {:>3}  {}  |  {}  ->  {}",
                entry.count,
                entry.item,
                entry.category,
                categorize(&entry.item)
            );
        }
    }

    let learned = Learned::new();
    let mut catalog_only: Vec<(&CorpusEntry, &str)> = corpus
        .iter()
        .map(|entry| {
            let category = catalog_category_with(&entry.item, &learned).unwrap_or("Other");
            (entry, category)
        })
        .filter(|(entry, category)| *category != entry.category)
        .collect();
    catalog_only.sort_by(|a, b| a.0.item.cmp(&b.0.item));
    let catalog_only_other = corpus
        .iter()
        .filter(|entry| catalog_category_with(&entry.item, &learned).unwrap_or("Other") == "Other")
        .count();
    println!(
        "\ncatalog only (no keyword rules): {} mismatches, {catalog_only_other} Other",
        catalog_only.len()
    );
    for (entry, category) in &catalog_only {
        println!(
            "  {:>3}  {}  |  {}  ->  {category}",
            entry.count, entry.item, entry.category
        );
    }

    assert!(
        mismatches.len() <= MAX_MISMATCHES,
        "distinct mismatches regressed: {} > {} (lower MAX_MISMATCHES once intentional)",
        mismatches.len(),
        MAX_MISMATCHES
    );
    assert!(
        other <= MAX_OTHER,
        "distinct 'Other' count regressed: {other} > {MAX_OTHER} (lower MAX_OTHER once intentional)"
    );
    assert!(
        weighted_mismatches <= MAX_WEIGHTED_MISMATCHES,
        "weighted mismatches regressed: {weighted_mismatches} > {MAX_WEIGHTED_MISMATCHES} (lower once intentional)"
    );
    assert!(
        weighted_other <= MAX_WEIGHTED_OTHER,
        "weighted 'Other' regressed: {weighted_other} > {MAX_WEIGHTED_OTHER} (lower once intentional)"
    );
    assert!(
        catalog_only.len() <= MAX_CATALOG_ONLY_MISMATCHES,
        "catalog-only mismatches regressed: {} > {MAX_CATALOG_ONLY_MISMATCHES}",
        catalog_only.len()
    );
    assert!(
        catalog_only_other <= MAX_CATALOG_ONLY_OTHER,
        "catalog-only 'Other' count regressed: {catalog_only_other} > {MAX_CATALOG_ONLY_OTHER}"
    );
}
