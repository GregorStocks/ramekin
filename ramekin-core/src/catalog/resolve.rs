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
    Modifiers,
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

/// Exact lookup, then singular/plural variants.
fn lookup(name: &str) -> Option<(Target, Via)> {
    if let Some(target) = CATALOG.index.get(name) {
        return Some((*target, Via::Exact));
    }
    let variants = [
        Some(format!("{name}s")),
        name.strip_suffix('s').map(str::to_string),
    ];
    variants
        .into_iter()
        .flatten()
        .find_map(|variant| CATALOG.index.get(&variant).copied())
        .map(|target| (target, Via::Plural))
}

/// Resolve a written ingredient name. An ambiguous exact match still gets a
/// second chance after modifier stripping, which may name a specific food.
pub fn resolve(item: &str) -> Resolution {
    let normalized = normalize(item);
    let first = lookup(&normalized);
    if let Some((Target::Entry(entry), via)) = first {
        return Resolution::Entry {
            entry: &CATALOG.entries[entry],
            via,
        };
    }
    let stripped = strip_modifiers(&normalized);
    let second = (stripped != normalized)
        .then(|| lookup(&stripped))
        .flatten();
    if let Some((Target::Entry(entry), _)) = second {
        return Resolution::Entry {
            entry: &CATALOG.entries[entry],
            via: Via::Modifiers,
        };
    }
    let is_ambiguous = |hit: Option<(Target, Via)>| matches!(hit, Some((Target::Ambiguous, _)));
    if is_ambiguous(first) || is_ambiguous(second) {
        Resolution::Ambiguous
    } else {
        Resolution::Unresolved
    }
}
