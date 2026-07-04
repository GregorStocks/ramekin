//! Relevance scoring for recipe search.
//!
//! This is the canonical implementation of search ranking, deliberately a
//! pure function over plain strings so that a client can mirror it for local
//! search later (see doc/client-logic-sharing.md and the
//! blocked-ios-local-search-relevance issue). The fixture corpus at
//! tests/fixtures/search_ranking/cases.json doubles as the shared test
//! vectors for any future client implementation — behavior changes here must
//! update that file, and vice versa.
//!
//! Matching semantics mirror the SQL filters in the list-recipes endpoint:
//! case-insensitive, accent-insensitive substring containment (Postgres
//! `f_unaccent(col) ILIKE f_unaccent(pattern)`). Scoring only *orders*
//! rows the SQL filter already matched, so a matched row may legitimately
//! score 0 (e.g. the SQL filter matched JSONB structure the scorer ignores);
//! callers break ties — including everything-scored-0 — by recency.

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// The searchable fields of one recipe, as plain text.
pub struct SearchDoc<'a> {
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub tags: &'a [String],
    /// One entry per ingredient text fragment (item, note, section).
    pub ingredients: &'a [String],
    pub instructions: &'a str,
    pub notes: Option<&'a str>,
}

/// An exact (normalized) title match for the whole query.
const WEIGHT_EXACT_TITLE: u32 = 100_000;
/// The whole query appears in the title as one phrase.
const WEIGHT_TITLE_PHRASE: u32 = 20_000;
/// Every token appears somewhere in the title.
const WEIGHT_ALL_TOKENS_IN_TITLE: u32 = 10_000;
/// Per-token weights by field.
const WEIGHT_TOKEN_IN_TITLE: u32 = 2_000;
const WEIGHT_TOKEN_IN_TAG: u32 = 800;
const WEIGHT_TOKEN_IN_DESCRIPTION: u32 = 400;
const WEIGHT_TOKEN_IN_INGREDIENT: u32 = 200;
const WEIGHT_TOKEN_IN_INSTRUCTIONS: u32 = 50;
const WEIGHT_TOKEN_IN_NOTES: u32 = 50;

/// Normalize text for matching: strip diacritics, then lowercase. This is
/// the Rust mirror of the database's `f_unaccent` + `ILIKE` semantics, so
/// "Crème Brûlée" and "creme brulee" normalize identically.
pub fn normalize_for_search(s: &str) -> String {
    s.nfd()
        .filter(|c| !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

/// Score one recipe against the text tokens of a search query. Tokens are
/// the plain-text terms of the query (filters like `tag:` already removed,
/// quoted phrases kept as single tokens), in the order the user typed them.
///
/// Higher is more relevant. Returns 0 for an empty token list.
pub fn relevance_score(text_tokens: &[String], doc: &SearchDoc) -> u32 {
    if text_tokens.is_empty() {
        return 0;
    }

    let tokens: Vec<String> = text_tokens
        .iter()
        .map(|t| normalize_for_search(t))
        .collect();
    let title = normalize_for_search(doc.title);
    let description = doc.description.map(normalize_for_search);
    let tags: Vec<String> = doc.tags.iter().map(|t| normalize_for_search(t)).collect();
    let ingredients: Vec<String> = doc
        .ingredients
        .iter()
        .map(|i| normalize_for_search(i))
        .collect();
    let instructions = normalize_for_search(doc.instructions);
    let notes = doc.notes.map(normalize_for_search);

    let mut score = 0u32;

    let phrase = tokens.join(" ");
    if title == phrase {
        score += WEIGHT_EXACT_TITLE;
    } else if title.contains(&phrase) {
        score += WEIGHT_TITLE_PHRASE;
    }

    if tokens.iter().all(|t| title.contains(t.as_str())) {
        score += WEIGHT_ALL_TOKENS_IN_TITLE;
    }

    for token in &tokens {
        let token = token.as_str();
        if title.contains(token) {
            score += WEIGHT_TOKEN_IN_TITLE;
        }
        if tags.iter().any(|t| t.contains(token)) {
            score += WEIGHT_TOKEN_IN_TAG;
        }
        if description.as_deref().is_some_and(|d| d.contains(token)) {
            score += WEIGHT_TOKEN_IN_DESCRIPTION;
        }
        if ingredients.iter().any(|i| i.contains(token)) {
            score += WEIGHT_TOKEN_IN_INGREDIENT;
        }
        if instructions.contains(token) {
            score += WEIGHT_TOKEN_IN_INSTRUCTIONS;
        }
        if notes.as_deref().is_some_and(|n| n.contains(token)) {
            score += WEIGHT_TOKEN_IN_NOTES;
        }
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc<'a>(title: &'a str, ingredients: &'a [String], instructions: &'a str) -> SearchDoc<'a> {
        SearchDoc {
            title,
            description: None,
            tags: &[],
            ingredients,
            instructions,
            notes: None,
        }
    }

    fn tokens(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn test_normalize_strips_accents_and_case() {
        assert_eq!(normalize_for_search("Crème Brûlée"), "creme brulee");
        assert_eq!(normalize_for_search("JALAPEÑO"), "jalapeno");
        assert_eq!(normalize_for_search("plain"), "plain");
    }

    #[test]
    fn test_exact_title_beats_phrase_in_title() {
        let q = tokens(&["garlic", "bread"]);
        let exact = relevance_score(&q, &doc("Garlic Bread", &[], ""));
        let phrase = relevance_score(&q, &doc("Cheesy Garlic Bread Bites", &[], ""));
        assert!(exact > phrase);
    }

    #[test]
    fn test_title_match_beats_body_match() {
        let q = tokens(&["garlic", "bread"]);
        let title_hit = relevance_score(&q, &doc("Garlic Bread", &[], ""));
        let body_hit = relevance_score(
            &q,
            &doc(
                "Roast Chicken",
                &["garlic".to_string(), "bread crumbs".to_string()],
                "Serve with bread.",
            ),
        );
        assert!(title_hit > body_hit);
        assert!(body_hit > 0);
    }

    #[test]
    fn test_empty_tokens_score_zero() {
        assert_eq!(relevance_score(&[], &doc("Garlic Bread", &[], "")), 0);
    }

    #[test]
    fn test_accent_insensitive_scoring() {
        let q = tokens(&["creme", "brulee"]);
        let hit = relevance_score(&q, &doc("Crème Brûlée", &[], ""));
        assert!(hit >= WEIGHT_EXACT_TITLE);
    }

    #[test]
    fn test_quoted_phrase_is_single_token() {
        // "green beans" arrives as one token; a title containing the words
        // in a different order must not get the phrase or exact bonus.
        let q = tokens(&["green beans"]);
        let in_order = relevance_score(&q, &doc("Green Beans Amandine", &[], ""));
        let out_of_order = relevance_score(&q, &doc("Beans, Green and Otherwise", &[], ""));
        assert!(in_order > out_of_order);
    }
}
