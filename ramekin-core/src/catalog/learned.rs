//! Names the committed catalog doesn't know, or calls ambiguous, resolved
//! later by an LLM and stored by the server (catalog step 3). The server loads
//! the stored answers for the names it's about to read into a `Learned` map;
//! everything here is pure, and a name with no answer yet stays unknown.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use super::{normalize, resolve, resolve_line, Resolution, Target, Via, CATALOG};

/// What a name the catalog doesn't know was resolved to.
#[derive(Debug, Clone, PartialEq)]
pub enum LearnedTarget {
    /// A catalog key (curated entry, USDA name, or alias) naming the same food
    /// or product; everything else comes from that entry. For an ambiguous
    /// name ("cheese") it's the food a recipe most likely means, so estimates
    /// say they assumed it.
    Entry(String),
    /// Not an ingredient (a heading, equipment, a serving note).
    NotFood,
    /// A real food no catalog entry matches, with the model's own numbers.
    /// Estimates label it; never used for categories or density alternatives.
    Estimate(EstimatedFood),
    /// The LLM couldn't tell; still unknown.
    Unknown,
}

/// A model's estimate for a food the catalog has no entry for.
#[derive(Debug, Clone, PartialEq)]
pub struct EstimatedFood {
    pub kcal_per_100g: f64,
    pub grams_per_cup: Option<f64>,
    /// One whole piece of a typical size, for a bare count.
    pub grams_per_piece: Option<f64>,
}

/// Learned answers keyed by the catalog-normalized name.
pub type Learned = HashMap<String, LearnedTarget>;

/// The normalized name to learn for an item, when the committed catalog
/// resolves nothing for it or calls it ambiguous ("cheese"), so the model
/// picks the food a recipe most likely means.
pub fn unlearned_name(item: &str) -> Option<String> {
    let name = normalize(item);
    (!name.is_empty()
        && matches!(
            resolve(&name),
            Resolution::Unresolved | Resolution::Ambiguous
        ))
    .then_some(name)
}

/// Whether the committed catalog calls this name ambiguous, so a learned
/// entry for it is an assumption to label.
pub fn is_ambiguous(item: &str) -> bool {
    matches!(resolve(item), Resolution::Ambiguous)
}

/// The model's estimate for a name no catalog entry matches.
pub fn learned_estimate<'a>(item: &str, learned: &'a Learned) -> Option<&'a EstimatedFood> {
    match learned.get(&normalize(item)) {
        Some(LearnedTarget::Estimate(estimate)) => Some(estimate),
        _ => None,
    }
}

/// Whether a stored learned key still names one catalog entry. The server
/// requeues answers whose key stopped doing so after a catalog change.
pub fn learned_key_resolves(key: &str) -> bool {
    matches!(resolve(key), Resolution::Entry { .. })
}

/// Like `resolve_line`, then a learned answer for a name the catalog doesn't
/// know or calls ambiguous. A learned key is resolved through the catalog, so
/// every attribute comes from committed data. An estimated food isn't a
/// catalog entry and stays `Unresolved` here (see `learned_estimate`).
pub fn resolve_line_with(item: &str, note: Option<&str>, learned: &Learned) -> Resolution {
    match resolve_line(item, note) {
        unknown @ (Resolution::Unresolved | Resolution::Ambiguous) => {
            resolve_learned(item, note, learned).unwrap_or(unknown)
        }
        resolved => resolved,
    }
}

/// `resolve`, then a learned answer.
pub fn resolve_with(item: &str, learned: &Learned) -> Resolution {
    match resolve(item) {
        unknown @ (Resolution::Unresolved | Resolution::Ambiguous) => {
            resolve_learned(item, None, learned).unwrap_or(unknown)
        }
        resolved => resolved,
    }
}

/// A learned key resolved like the line itself, note included, so "cooked"
/// still selects the cooked food (or stays unknown) rather than the dry one.
/// None leaves the committed answer.
fn resolve_learned(item: &str, note: Option<&str>, learned: &Learned) -> Option<Resolution> {
    match learned.get(&normalize(item))? {
        LearnedTarget::Entry(key) => match resolve_line(key, note) {
            Resolution::Entry { entry, .. } => Some(Resolution::Entry {
                entry,
                via: Via::Learned,
            }),
            // The line's note rules the entry out (e.g. cooked rice with no
            // cooked entry). Keys that stopped resolving at all are requeued
            // by the server at startup.
            _ => None,
        },
        LearnedTarget::NotFood => Some(Resolution::NotFood),
        LearnedTarget::Estimate(_) | LearnedTarget::Unknown => None,
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
