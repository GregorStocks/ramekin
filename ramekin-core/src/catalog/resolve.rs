//! Turn a written ingredient name into a catalog entry.

use super::{normalize, Entry, Target, CATALOG};

/// What a written ingredient name refers to.
#[derive(Debug)]
pub enum Resolution {
    Entry {
        entry: &'static Entry,
        via: Via,
    },
    /// The name could mean several foods (e.g. "cheese"), so no attribute is
    /// reported for it.
    Ambiguous,
    Unresolved,
}

/// Which lookup step matched, for debugging and audits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Exact,
    Plural,
    /// After stripping temperature or preparation modifiers.
    Modifiers,
    /// After dropping trailing ", ..." clauses.
    Clause,
    /// After dropping leading size or preparation words.
    LeadingModifiers,
}

/// Temperature and preparation modifiers stripped before a second lookup.
const MODIFIERS_TO_STRIP: &[&str] = &[
    "room temperature ",
    "cold ",
    "warm ",
    "melted ",
    "softened ",
    ", softened",
    ", melted",
    ", cold",
    ", at room temperature",
    ", room temperature",
    ", chilled",
    ", sifted",
];

fn strip_modifiers(name: &str) -> String {
    let mut result = name.to_string();
    for modifier in MODIFIERS_TO_STRIP {
        if let Some(stripped) = result.strip_prefix(modifier) {
            result = stripped.to_string();
        }
        if let Some(stripped) = result.strip_suffix(modifier) {
            result = stripped.to_string();
        }
    }
    result
}

/// Leading words that describe size or preparation, never which food it is.
/// Words that change the food are deliberately absent: "ground" (ground beef),
/// "dried" (dried apricots), "light" (light cream), "crushed" (crushed
/// tomatoes), "whole" (whole chicken).
const LEADING_MODIFIERS: &[&str] = &[
    "boneless",
    "chopped",
    "coarsely",
    "diced",
    "extra-large",
    "finely",
    "fresh",
    "freshly",
    "grated",
    "large",
    "medium",
    "minced",
    "organic",
    "packed",
    "peeled",
    "roughly",
    "shredded",
    "skinless",
    "sliced",
    "small",
    "thinly",
    "toasted",
];

/// The name with leading size/preparation words dropped one at a time,
/// including comma-separated ones: "grated fresh ginger" -> ["fresh ginger",
/// "ginger"]. Trying each step lets a more specific name ("fresh ginger") win
/// before a bare one ("ginger").
fn leading_strips(name: &str) -> Vec<String> {
    let words: Vec<&str> = name.split(' ').collect();
    let strippable = words
        .iter()
        .take_while(|word| LEADING_MODIFIERS.contains(&word.trim_end_matches(',')))
        .count();
    (1..=strippable)
        .filter_map(|count| words.get(count..))
        .map(|rest| rest.join(" "))
        .filter(|rest| !rest.is_empty())
        .collect()
}

/// Whether an entry is a dried or ground form ("spices, sage, ground", "dried
/// oregano"). Dropping "fresh", "chopped", or "grated" must not land on one,
/// since fresh and dried herbs differ in both density and calories.
fn is_dried_form(entry: &Entry) -> bool {
    entry.id.starts_with("spices, ") || entry.id.contains("dried") || entry.id.ends_with(", ground")
}

/// The name with trailing ", ..." or "; ..." clauses removed one at a time,
/// longest prefix first ("a, b; c" -> ["a, b", "a"]).
fn clause_prefixes(name: &str) -> Vec<&str> {
    let mut prefixes = Vec::new();
    let mut rest = name;
    while let Some(index) = rest.rfind([',', ';']) {
        rest = rest
            .get(..index)
            .expect("a matched ASCII delimiter is a character boundary")
            .trim_end();
        if rest.is_empty() {
            break;
        }
        prefixes.push(rest);
    }
    prefixes
}

/// Singular/plural spellings to try when the name itself is unknown. The
/// plain "s" forms come first, matching the original density lookup.
fn plural_variants(name: &str) -> Vec<String> {
    let mut variants = vec![format!("{name}s")];
    variants.extend(name.strip_suffix('s').map(str::to_string));
    variants.push(format!("{name}es"));
    variants.extend(name.strip_suffix("es").map(str::to_string));
    variants.extend(name.strip_suffix("ies").map(|stem| format!("{stem}y")));
    variants.extend(name.strip_suffix('y').map(|stem| format!("{stem}ies")));
    variants
}

/// Exact lookup, then singular/plural variants.
fn lookup(name: &str) -> Option<(Target, Via)> {
    if let Some(target) = CATALOG.index.get(name) {
        return Some((*target, Via::Exact));
    }
    plural_variants(name)
        .iter()
        .find_map(|variant| CATALOG.index.get(variant).copied())
        .map(|target| (target, Via::Plural))
}

/// Resolve a written ingredient name, trying progressively looser spellings.
/// Only a match to a specific food ends the search: an ambiguous match (e.g.
/// "cheese") still lets a later spelling name a specific food, and trimming a
/// clause never settles for a name that isn't a food.
pub fn resolve(item: &str) -> Resolution {
    let normalized = normalize(item);

    let mut candidates: Vec<(String, Option<Via>)> = vec![
        (normalized.clone(), None),
        (strip_modifiers(&normalized), Some(Via::Modifiers)),
    ];
    for prefix in clause_prefixes(&normalized) {
        candidates.push((prefix.to_string(), Some(Via::Clause)));
        candidates.push((strip_modifiers(prefix), Some(Via::Clause)));
    }
    for stripped in leading_strips(&normalized) {
        let prefixes: Vec<String> = clause_prefixes(&stripped)
            .into_iter()
            .map(str::to_string)
            .collect();
        candidates.push((stripped, Some(Via::LeadingModifiers)));
        candidates.extend(
            prefixes
                .into_iter()
                .map(|prefix| (prefix, Some(Via::LeadingModifiers))),
        );
    }
    let says_dried = normalized.contains("dried") || normalized.contains("ground");

    let mut ambiguous = false;
    let mut tried = std::collections::HashSet::new();
    for (name, step) in candidates {
        if !tried.insert(name.clone()) {
            continue;
        }
        match lookup(&name) {
            Some((Target::Entry(entry), via)) => {
                let entry = &CATALOG.entries[entry];
                // A bare name reached by dropping "fresh" or "chopped" must not become
                // the dried spice; a curated alias that keeps a prep word (e.g.
                // "grated nutmeg") is an explicit choice and still counts.
                let bare = !name
                    .split(' ')
                    .any(|word| LEADING_MODIFIERS.contains(&word.trim_end_matches(',')));
                if step == Some(Via::LeadingModifiers)
                    && bare
                    && !says_dried
                    && is_dried_form(entry)
                {
                    continue;
                }
                return Resolution::Entry {
                    entry,
                    via: step.unwrap_or(via),
                };
            }
            Some((Target::Ambiguous, _)) => ambiguous = true,
            None => {}
        }
    }
    if ambiguous {
        Resolution::Ambiguous
    } else {
        Resolution::Unresolved
    }
}
