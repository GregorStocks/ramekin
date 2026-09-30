//! Names the committed catalog doesn't know, resolved later by an LLM and
//! stored by the server (catalog step 3). The server loads the stored answers
//! for the names it's about to read into a `Learned` map; everything here is
//! pure, and a name with no answer yet stays unknown.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use super::{normalize, resolve, resolve_line, Resolution, Target, Via, CATALOG};

/// What a name the catalog doesn't know was resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearnedTarget {
    /// A catalog key (curated entry, USDA name, or alias) naming the same food
    /// or product; everything else comes from that entry.
    Entry(String),
    /// Not an ingredient (a heading, equipment, a serving note).
    NotFood,
    /// The LLM couldn't tell; still unknown.
    Unknown,
}

/// Learned answers keyed by the catalog-normalized name.
pub type Learned = HashMap<String, LearnedTarget>;

/// The normalized name to learn for an item, when the committed catalog
/// resolves nothing for it. Ambiguous names ("cheese") are deliberate and
/// never learned.
pub fn unlearned_name(item: &str) -> Option<String> {
    let name = normalize(item);
    (!name.is_empty() && matches!(resolve(&name), Resolution::Unresolved)).then_some(name)
}

/// Like `resolve_line`, then a learned answer for a name the catalog doesn't
/// know. A learned key is resolved through the catalog, so every attribute
/// comes from committed data.
pub fn resolve_line_with(item: &str, note: Option<&str>, learned: &Learned) -> Resolution {
    match resolve_line(item, note) {
        Resolution::Unresolved => resolve_learned(item, note, learned),
        resolved => resolved,
    }
}

/// `resolve`, then a learned answer.
pub fn resolve_with(item: &str, learned: &Learned) -> Resolution {
    match resolve(item) {
        Resolution::Unresolved => resolve_learned(item, None, learned),
        resolved => resolved,
    }
}

/// A learned key resolved like the line itself, note included, so "cooked"
/// still selects the cooked food (or stays unknown) rather than the dry one.
fn resolve_learned(item: &str, note: Option<&str>, learned: &Learned) -> Resolution {
    match learned.get(&normalize(item)) {
        Some(LearnedTarget::Entry(key)) => match resolve_line(key, note) {
            Resolution::Entry { entry, .. } => Resolution::Entry {
                entry,
                via: Via::Learned,
            },
            // The key no longer names a single entry (the catalog changed).
            _ => Resolution::Unresolved,
        },
        Some(LearnedTarget::NotFood) => Resolution::NotFood,
        Some(LearnedTarget::Unknown) | None => Resolution::Unresolved,
    }
}

/// Words that don't say which food a name is.
const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "the", "of", "or", "for", "with", "to", "in", "fresh", "chopped", "sliced",
    "diced", "minced", "large", "small", "medium", "raw", "whole",
];

fn tokens(name: &str) -> HashSet<String> {
    name.split(|c: char| !c.is_alphanumeric())
        .filter(|word| word.len() > 1 && !STOP_WORDS.contains(word))
        .map(|word| {
            word.strip_suffix('s')
                .filter(|w| w.len() > 2)
                .unwrap_or(word)
                .to_string()
        })
        .collect()
}

/// Every catalog key naming one entry, with its tokens.
static KEYS: LazyLock<Vec<(&'static str, HashSet<String>)>> = LazyLock::new(|| {
    let mut keys: Vec<_> = CATALOG
        .index
        .iter()
        .filter(|(_, target)| matches!(target, Target::Entry(_)))
        .map(|(key, _)| (key.as_str(), tokens(key)))
        .collect();
    keys.sort_by(|a, b| a.0.cmp(b.0));
    keys
});

/// Up to `limit` catalog keys that share words with `name`, best first: most
/// shared words, then the shortest key. The LLM may only answer with one of
/// these, which keeps its prompt small and its answers inside the catalog.
pub fn candidates(name: &str, limit: usize) -> Vec<&'static str> {
    let wanted = tokens(&normalize(name));
    if wanted.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(usize, usize, &'static str)> = KEYS
        .iter()
        .filter_map(|(key, key_tokens)| {
            let shared = key_tokens.intersection(&wanted).count();
            (shared > 0).then_some((shared, key_tokens.len(), *key))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(b.2)));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, _, key)| key)
        .collect()
}
