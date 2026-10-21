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
//! reports the items it puts in the wrong aisle and those it leaves in "Other". It
//! reports two ways: per distinct item, and weighted by usage count — so a
//! regression on frequently-added items can't hide behind improvements on one-off
//! items. It is a regression guard via upper bounds on those counts: improvements
//! keep passing, regressions fail. Tighten the bounds as the categorizer improves.
//!
//! Run `make shopping-list-categorizer-test` to see the full report.

use ramekin_core::ingredient_categorizer::{categorize, CATEGORIES};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Upper bounds, at the current measured values; lower them as the
/// categorizer improves. A wrong aisle (a category other than the label) is
/// counted apart from no aisle ("Other" for an item labeled with an aisle):
/// a wrong answer misleads the shopper, while a missing one is what the AI name
/// resolver fills in on the server, which this offline test runs without.
/// Distinct items given a wrong aisle.
const MAX_WRONG_AISLE: usize = 2;
/// Distinct items labeled with an aisle that get "Other".
const MAX_NO_AISLE: usize = 13;
/// Usage-weighted (times the item was added) wrong and missing aisles.
const MAX_WEIGHTED_WRONG_AISLE: u64 = 2;
const MAX_WEIGHTED_NO_AISLE: u64 = 14;

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
    let mut wrong: Vec<(&CorpusEntry, &str)> = Vec::new();
    let mut missing: Vec<&CorpusEntry> = Vec::new();
    for entry in &corpus {
        match categorize(&entry.item) {
            actual if actual == entry.category => {}
            "Other" => missing.push(entry),
            actual => wrong.push((entry, actual)),
        }
    }
    wrong.sort_by(|a, b| a.0.item.cmp(&b.0.item));
    missing.sort_by(|a, b| a.item.cmp(&b.item));
    let weighted_wrong: u64 = wrong.iter().map(|(e, _)| e.count).sum();
    let weighted_missing: u64 = missing.iter().map(|e| e.count).sum();
    let correct = total - wrong.len() - missing.len();

    println!("\n=== Shopping-list categorizer scorecard ===");
    println!("distinct items: {total}");
    println!(
        "  correct:     {correct} ({:.1}%)",
        100.0 * correct as f64 / total as f64
    );
    println!("  wrong aisle: {}", wrong.len());
    println!("  no aisle:    {}", missing.len());
    println!("usage-weighted ({weighted_total} adds):");
    println!("  wrong aisle: {weighted_wrong}");
    println!("  no aisle:    {weighted_missing}");
    if !wrong.is_empty() {
        println!("\n--- wrong aisle (count | item | expected -> got) ---");
        for (entry, actual) in &wrong {
            println!(
                "  {:>3}  {}  |  {}  ->  {actual}",
                entry.count, entry.item, entry.category
            );
        }
    }
    if !missing.is_empty() {
        println!("\n--- no aisle (count | item | expected) ---");
        for entry in &missing {
            println!(
                "  {:>3}  {}  |  {}",
                entry.count, entry.item, entry.category
            );
        }
    }

    assert!(
        wrong.len() <= MAX_WRONG_AISLE,
        "wrong aisles regressed: {} > {MAX_WRONG_AISLE} (lower the bound once intentional)",
        wrong.len()
    );
    assert!(
        missing.len() <= MAX_NO_AISLE,
        "missing aisles regressed: {} > {MAX_NO_AISLE} (lower the bound once intentional)",
        missing.len()
    );
    assert!(
        weighted_wrong <= MAX_WEIGHTED_WRONG_AISLE,
        "weighted wrong aisles regressed: {weighted_wrong} > {MAX_WEIGHTED_WRONG_AISLE}"
    );
    assert!(
        weighted_missing <= MAX_WEIGHTED_NO_AISLE,
        "weighted missing aisles regressed: {weighted_missing} > {MAX_WEIGHTED_NO_AISLE}"
    );
}
