//! Relevance scoring for recipe search.
//!
//! This is the canonical implementation of search ranking, deliberately a
//! pure function over plain strings so that a client can mirror it for local
//! search (see doc/client-logic-sharing.md). The shared vectors at
//! shared-test-vectors/search-ranking.json double as the test corpus for
//! client implementations — behavior changes here must update that file, and
//! vice versa.
//!
//! Matching semantics mirror the SQL filters in the list-recipes endpoint:
//! case-insensitive, accent-insensitive substring containment (Postgres
//! `f_unaccent(col) ILIKE f_unaccent(pattern)`). `normalize_for_search`
//! reproduces that pipeline exactly by consuming the versioned contract at
//! shared-test-vectors/search-normalization.json: the server database's
//! complete per-codepoint unaccent dictionary followed by its per-codepoint
//! lower() mapping (which is what ILIKE's case-insensitivity applies).
//! tests/test_search_normalization_contract.py fails when that asset drifts
//! from the running database, and recipe sync refuses to feed a client whose
//! contract version differs, so local matching can rely on this
//! normalization being byte-identical to SQL matching.

use std::collections::HashMap;
use std::sync::OnceLock;

/// The searchable fields of one recipe, as plain text.
pub struct SearchDoc<'a> {
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub tags: &'a [String],
    /// One entry per ingredient: its full text (measurement amounts/units,
    /// item, note, section), so unit/amount tokens score like any other.
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

#[derive(serde::Deserialize)]
struct ContractFile {
    version: u32,
    unaccent: HashMap<String, String>,
    lower: HashMap<String, String>,
}

struct Contract {
    version: u32,
    unaccent: HashMap<char, String>,
    lower: HashMap<char, char>,
}

fn parse_codepoint_key(key: &str) -> char {
    let cp = u32::from_str_radix(key, 16)
        .unwrap_or_else(|_| panic!("invalid codepoint key in normalization contract: {key}"));
    char::from_u32(cp).unwrap_or_else(|| {
        panic!("codepoint key is not a scalar value in normalization contract: {key}")
    })
}

fn contract() -> &'static Contract {
    static CONTRACT: OnceLock<Contract> = OnceLock::new();
    CONTRACT.get_or_init(|| {
        let file: ContractFile = serde_json::from_str(include_str!(
            "../../shared-test-vectors/search-normalization.json"
        ))
        .expect("search-normalization.json is invalid");
        Contract {
            version: file.version,
            unaccent: file
                .unaccent
                .into_iter()
                .map(|(key, replacement)| (parse_codepoint_key(&key), replacement))
                .collect(),
            lower: file
                .lower
                .into_iter()
                .map(|(key, replacement)| {
                    let mut chars = replacement.chars();
                    let (Some(lower), None) = (chars.next(), chars.next()) else {
                        panic!("lower mapping for {key} is not a single character");
                    };
                    (parse_codepoint_key(&key), lower)
                })
                .collect(),
        }
    })
}

/// The version of the shared normalization contract this build consumes.
/// Carried in the recipe sync response so a client with a different contract
/// version fails sync instead of silently mismatching server search results.
pub fn normalization_contract_version() -> u32 {
    contract().version
}

/// Normalize text for matching and scoring, byte-identical to the database's
/// `f_unaccent(text)` under `ILIKE`: apply the contract's unaccent mapping
/// per codepoint (replacements may be empty or multi-character), then its
/// per-codepoint lower() mapping. Unlike Rust's `str::to_lowercase`, the
/// database lowercases without context (no final-sigma handling), so the
/// contract's table is authoritative.
pub fn normalize_for_search(s: &str) -> String {
    let contract = contract();
    let lower = |c: char| contract.lower.get(&c).copied().unwrap_or(c);
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match contract.unaccent.get(&c) {
            Some(replacement) => out.extend(replacement.chars().map(lower)),
            None => out.push(lower(c)),
        }
    }
    out
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
    fn test_contract_version_is_positive() {
        assert!(normalization_contract_version() >= 1);
    }

    #[test]
    fn test_normalize_strips_accents_and_case() {
        assert_eq!(normalize_for_search("Crème Brûlée"), "creme brulee");
        assert_eq!(normalize_for_search("JALAPEÑO"), "jalapeno");
        assert_eq!(normalize_for_search("plain"), "plain");
    }

    #[test]
    fn test_normalize_expands_ligature_letters() {
        assert_eq!(normalize_for_search("Œufs"), "oeufs");
        assert_eq!(normalize_for_search("Æbleskiver"), "aebleskiver");
        assert_eq!(
            normalize_for_search("Spätzle mit Soße"),
            "spatzle mit sosse"
        );
        assert_eq!(normalize_for_search("Smørrebrød"), "smorrebrod");
    }

    #[test]
    fn test_normalize_expands_presentation_forms_and_punctuation() {
        // The unaccent dictionary expands presentation ligatures and vulgar
        // fractions, and folds typographic punctuation to ASCII.
        assert_eq!(normalize_for_search("ﬁnely chopped"), "finely chopped");
        assert_eq!(normalize_for_search("1½ cups"), "11/2 cups");
        assert_eq!(normalize_for_search("Mom’s Apple Cake"), "mom's apple cake");
        assert_eq!(normalize_for_search("Sweet–and–Sour"), "sweet-and-sour");
    }

    #[test]
    fn test_normalize_deletes_combining_marks() {
        // Decomposed "é" (e + U+0301): the dictionary deletes the bare
        // combining mark, exactly like the database.
        assert_eq!(normalize_for_search("Cre\u{301}me"), "creme");
    }

    #[test]
    fn test_normalize_folds_fullwidth_forms() {
        assert_eq!(normalize_for_search("ＡＢＣ"), "abc");
    }

    #[test]
    fn test_normalize_lowercases_per_codepoint_without_context() {
        // The database's lower() has no final-sigma special case: Σ always
        // lowercases to σ, and ς stays ς. Rust's to_lowercase would produce
        // "ας" for "ΑΣ"; the contract must win.
        assert_eq!(normalize_for_search("ΣΟΥΠΑ"), "σουπα");
        assert_eq!(normalize_for_search("σουπες"), "σουπες");
    }

    #[test]
    fn test_ligature_title_gets_exact_match_score() {
        let q = tokens(&["oeufs", "en", "meurette"]);
        let hit = relevance_score(&q, &doc("Œufs en Meurette", &[], ""));
        assert!(hit >= WEIGHT_EXACT_TITLE);
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
