//! Shared-vector tests for search relevance ranking.
//!
//! The corpus in `fixtures/search_ranking/cases.json` is the canonical
//! specification of ranking behavior. Each case lists recipe ids in expected
//! score order and requires the scores to be strictly decreasing, so ranking
//! never depends on tie-breaking; `zero_score` pins ids that must not match
//! at all. A future client-side search implementation must consume this same
//! file and produce identical rankings (see
//! issues/blocked-ios-local-search-relevance.json5).

use ramekin_core::search::{relevance_score, SearchDoc};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct FixtureFile {
    recipes: Vec<FixtureRecipe>,
    cases: Vec<FixtureCase>,
}

#[derive(Debug, Deserialize)]
struct FixtureRecipe {
    id: String,
    title: String,
    description: Option<String>,
    tags: Vec<String>,
    ingredients: Vec<String>,
    instructions: String,
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FixtureCase {
    name: String,
    tokens: Vec<String>,
    expected_order: Vec<String>,
    zero_score: Vec<String>,
}

fn score(recipe: &FixtureRecipe, tokens: &[String]) -> u32 {
    relevance_score(
        tokens,
        &SearchDoc {
            title: &recipe.title,
            description: recipe.description.as_deref(),
            tags: &recipe.tags,
            ingredients: &recipe.ingredients,
            instructions: &recipe.instructions,
            notes: recipe.notes.as_deref(),
        },
    )
}

#[test]
fn test_search_ranking_vectors() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/search_ranking/cases.json");
    let fixture: FixtureFile =
        serde_json::from_str(&fs::read_to_string(&path).expect("read fixture"))
            .expect("parse fixture");

    let by_id: HashMap<&str, &FixtureRecipe> =
        fixture.recipes.iter().map(|r| (r.id.as_str(), r)).collect();

    for case in &fixture.cases {
        let scores: Vec<(&str, u32)> = case
            .expected_order
            .iter()
            .map(|id| {
                let recipe = by_id
                    .get(id.as_str())
                    .unwrap_or_else(|| panic!("case '{}': unknown recipe id '{}'", case.name, id));
                (id.as_str(), score(recipe, &case.tokens))
            })
            .collect();

        for pair in scores.windows(2) {
            assert!(
                pair[0].1 > pair[1].1,
                "case '{}': expected '{}' (score {}) to outrank '{}' (score {})",
                case.name,
                pair[0].0,
                pair[0].1,
                pair[1].0,
                pair[1].1
            );
        }
        if let Some(last) = scores.last() {
            assert!(
                last.1 > 0,
                "case '{}': '{}' is in expected_order but scored 0",
                case.name,
                last.0
            );
        }

        for id in &case.zero_score {
            let recipe = by_id
                .get(id.as_str())
                .unwrap_or_else(|| panic!("case '{}': unknown recipe id '{}'", case.name, id));
            let s = score(recipe, &case.tokens);
            assert_eq!(
                s, 0,
                "case '{}': expected '{}' to score 0, got {}",
                case.name, id, s
            );
        }
    }
}
