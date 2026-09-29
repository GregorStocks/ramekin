//! Keeping curated names in the form the ingredient parser produces.
//!
//! Classification passes used to alias whatever string the parser left in
//! `item`, including amounts and notes it failed to split off ("about 7 cloves
//! garlic", "chickpeas, rinsed"). Once the parser splits them, those keys are
//! dead weight. `clean_curated` re-parses every alias and not-food key and:
//! - removes it when the parsed name already resolves the same way;
//! - renames it to the parsed name when that name resolves to nothing;
//! - otherwise reports a conflict and keeps it.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::{normalize, resolve, Resolution};
use crate::ingredient_parser::parse_ingredient;

#[derive(Debug, PartialEq, Eq)]
pub enum CuratedChange {
    /// The parsed name already resolves the same way.
    Remove { section: &'static str, name: String },
    /// The parsed name resolves to nothing, so the key moves there.
    Rename {
        section: &'static str,
        from: String,
        to: String,
    },
    /// The parsed name resolves differently; kept for a person to decide.
    Conflict {
        section: &'static str,
        name: String,
        parsed: String,
        detail: String,
    },
}

/// The item the parser stores for a name read as an ingredient line, or the
/// name itself when re-parsing would cut into it. A bare item can start with
/// a number that is part of the name ("5- to 6-inch chiles", "85% lean ground
/// beef", "1/4-inch-thick slices"), so a parse only counts when what is left
/// starts with a word.
pub fn parsed_name(name: &str) -> String {
    let parsed = normalize(&parse_ingredient(name).item);
    let starts_with_word = parsed.chars().next().is_some_and(char::is_alphabetic);
    if parsed.is_empty() || !starts_with_word || parsed.starts_with("percent ") {
        return normalize(name);
    }
    parsed
}

/// Whether re-parsing drops text that could say which food the name meant:
/// examples ("such as basil"), alternatives ("or cod"), or a personal choice
/// ("i used manila"). Moving such a key to the shorter name would turn a
/// one-off choice into the meaning of a generic name ("fresh herbs").
fn drops_identity(name: &str) -> bool {
    const IDENTITY_MARKERS: &[&str] = &[
        "and/or",
        "any ",
        "e.g",
        "example",
        "i use",
        "like ",
        "mix of",
        "or ",
        "preferably",
        "such as",
        "your choice",
        "your favorite",
    ];
    let parsed = parse_ingredient(name);
    parsed.note.is_some_and(|note| {
        let note = note.to_lowercase();
        IDENTITY_MARKERS.iter().any(|marker| note.contains(marker))
    })
}

/// Whether the parsed name is a safe new key: only notes were split off (no
/// amount, which means the key was a fragment of a line), and what's left
/// isn't itself a fragment ("pepper or", "and half", "plus 2 tablespoons
/// lard").
fn is_clean_rename(name: &str, parsed: &str) -> bool {
    const FRAGMENT_EDGES: &[&str] = &["and", "more", "or", "plus", "to", "per", "of", "with"];
    let words: Vec<&str> = parsed.split_whitespace().collect();
    parse_ingredient(name).measurements.is_empty()
        && !words.first().is_some_and(|w| FRAGMENT_EDGES.contains(w))
        && !words.last().is_some_and(|w| FRAGMENT_EDGES.contains(w))
}

/// How a name resolves, comparably: the entry id, or the kind of non-match.
fn outcome(name: &str) -> String {
    match resolve(name) {
        Resolution::Entry { entry, .. } => format!("entry {}", entry.id),
        Resolution::Compound(entries) => format!(
            "compound {}",
            entries
                .iter()
                .map(|e| e.id.as_str())
                .collect::<Vec<_>>()
                .join(" + ")
        ),
        Resolution::NotFood => "not food".to_string(),
        Resolution::Ambiguous => "ambiguous".to_string(),
        Resolution::Unresolved => "unresolved".to_string(),
    }
}

/// Plan and apply the cleanup to curated.json's text. Returns the new text
/// (formatted like the file: 2-space indent, sorted keys) and the changes.
pub fn clean_curated(json: &str) -> (String, Vec<CuratedChange>) {
    let mut curated: Map<String, Value> =
        serde_json::from_str(json).expect("curated.json is an object");
    let mut changes = Vec::new();
    for section in ["aliases", "not_food"] {
        let names: Map<String, Value> = curated[section]
            .as_object()
            .expect("curated section is an object")
            .clone();
        // Keys whose parsed name resolves to nothing, grouped by that name:
        // one key moves there only if every key in the group agrees.
        let mut renames: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for name in names.keys() {
            let parsed = parsed_name(name);
            if parsed.is_empty() || parsed == *name {
                continue;
            }
            let current = outcome(name);
            let cleaned = outcome(&parsed);
            if cleaned == current {
                changes.push(CuratedChange::Remove {
                    section,
                    name: name.clone(),
                });
            } else if cleaned == "unresolved"
                && (drops_identity(name)
                    || !is_clean_rename(name, &parsed)
                    || names[name].is_null())
            {
                // Moving the key would broaden a one-off choice (an example,
                // an ambiguity) to a shorter name, or key on a fragment.
                changes.push(CuratedChange::Conflict {
                    section,
                    name: name.clone(),
                    parsed,
                    detail: "not safe to move to the parsed name".to_string(),
                });
            } else if cleaned == "unresolved" {
                renames.entry(parsed).or_default().push(name.clone());
            } else {
                changes.push(CuratedChange::Conflict {
                    section,
                    name: name.clone(),
                    parsed,
                    detail: format!("{current} vs {cleaned}"),
                });
            }
        }
        for (parsed, group) in renames {
            let first = &group[0];
            if group.iter().all(|name| names[name] == names[first]) {
                changes.push(CuratedChange::Rename {
                    section,
                    from: first.clone(),
                    to: parsed.clone(),
                });
                for name in &group[1..] {
                    changes.push(CuratedChange::Remove {
                        section,
                        name: name.clone(),
                    });
                }
            } else {
                for name in group {
                    changes.push(CuratedChange::Conflict {
                        section,
                        name,
                        parsed: parsed.clone(),
                        detail: "keys with this parsed name disagree".to_string(),
                    });
                }
            }
        }
        let map = curated[section].as_object_mut().expect("object");
        for change in &changes {
            match change {
                CuratedChange::Remove { section: s, name } if *s == section => {
                    map.remove(name);
                }
                CuratedChange::Rename {
                    section: s,
                    from,
                    to,
                } if *s == section => {
                    let value = map.remove(from).expect("renamed key exists");
                    map.insert(to.clone(), value);
                }
                _ => {}
            }
        }
    }
    let text = serde_json::to_string_pretty(&Value::Object(curated)).expect("serializes") + "\n";
    (text, changes)
}
