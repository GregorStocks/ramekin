//! Keeping curated names in the form the ingredient parser produces.
//!
//! Classification passes used to alias whatever string the parser left in
//! `item`, including amounts and notes it failed to split off ("about 7 cloves
//! garlic", "chickpeas, rinsed"). Once the parser splits them, those keys are
//! dead weight. `clean_curated` removes keys starting with the tail of a
//! hyphenated amount ("-ounce block feta"), re-parses every other alias and
//! not-food key and:
//! - removes it when the parsed name already resolves the same way;
//! - renames it to the parsed name when that name resolves to nothing;
//! - otherwise reports a conflict and keeps it.
//!
//! It then removes every alias and not-food key the resolver no longer needs:
//! hidden from the catalog, the key and every corpus name that could reach it
//! still resolve the same way, through modifier stripping, plurals or a
//! shorter key ("chopped fresh basil").
//!
//! Prod lines are never re-parsed, so keys prod's stored items still hit
//! (`PROD_NAMES`) are left alone.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use serde_json::{Map, Value};

use super::resolve::{chosen_alternative, resolve, with_hidden};
use super::{normalize, resolve_line, Resolution, BESPOKE_JSON, CATALOG};
use crate::ingredient_parser::parse_ingredient;

#[derive(Debug, PartialEq, Eq)]
pub enum CuratedChange {
    /// The parsed name already resolves the same way.
    Remove { section: &'static str, name: String },
    /// Hidden from the catalog, the name still resolves the same way.
    Redundant { section: &'static str, name: String },
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
/// name itself when re-parsing would cut into it (see `reparse_item`).
///
/// A name a measured line keeps whole is already what the parser stores, even
/// if the bare name reads differently: alone, "medium grain rice" and "small
/// new potatoes" lose their first word as a size, but "1 cup medium grain
/// rice" keeps it.
pub fn parsed_name(name: &str) -> String {
    let normalized = normalize(name);
    if normalize(&parse_ingredient(&format!("1 cup {name}")).item) == normalized {
        return normalized;
    }
    crate::ingredient_parser::reparse_item(name)
        .map(|parsed| normalize(&parsed.item))
        .unwrap_or(normalized)
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
    describe(resolve(name))
}

fn describe(resolution: Resolution) -> String {
    match resolution {
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
        Resolution::Ambiguous(_) => "ambiguous".to_string(),
        Resolution::Unresolved => "unresolved".to_string(),
    }
}

/// Everything a calorie estimate takes from the catalog for one line, without
/// the catalog names in `hidden`: the food its item and note resolve to, the
/// item's own resolution (an ambiguous item is reported as such), and the
/// alternative it says it assumed.
fn line_outcome(line: &(String, Option<String>), hidden: &HashSet<String>) -> String {
    let (item, note) = line;
    with_hidden(hidden, || {
        format!(
            "{} / {} / {:?}",
            describe(resolve_line(item, note.as_deref())),
            outcome(item),
            chosen_alternative(item)
        )
    })
}

/// Plan and apply the cleanup to curated.json's text. Returns the new text
/// (formatted like the file: 2-space indent, sorted keys) and the changes.
///
/// A key among `known.verbatim` stays even when the parser would never
/// produce it: those names are matched without parsing. A key among
/// `known.prod` is never touched. No key goes or moves while a line in
/// `known.corpus` estimates differently without it.
pub fn clean_curated(json: &str, known: &CleanupNames) -> (String, Vec<CuratedChange>) {
    let mut curated: Map<String, Value> =
        serde_json::from_str(json).expect("curated.json is an object");
    let mut changes = Vec::new();
    let mut reach = Reach::new(&known.corpus);
    for section in ["aliases", "not_food"] {
        let names: Map<String, Value> = curated[section]
            .as_object()
            .expect("curated section is an object")
            .clone();
        // Keys whose parsed name resolves to nothing, grouped by that name:
        // one key moves there only if every key in the group agrees.
        let mut renames: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for name in names.keys() {
            // The tail of a hyphenated amount the parser once split ("-ounce
            // block feta" from "8-ounce block feta"); it now reads the whole
            // amount, so no line produces the key.
            if known.prod.contains(name) {
                continue;
            }
            if name.starts_with('-') && !known.verbatim.contains(name) {
                changes.push(CuratedChange::Remove {
                    section,
                    name: name.clone(),
                });
                continue;
            }
            let parsed = parsed_name(name);
            if parsed.is_empty() || parsed == *name || known.verbatim.contains(name) {
                continue;
            }
            let current = outcome(name);
            let cleaned = outcome(&parsed);
            // A stored line that reaches the key another way ("fresh
            // katakuriko (potato starch" through modifier stripping) still
            // needs it; lines that are the key itself were judged above.
            let indirect = reach
                .reaching(name)
                .into_iter()
                .filter(|(item, _)| normalize(item) != *name)
                .cloned()
                .collect();
            if (cleaned == current || cleaned == "unresolved")
                && !reach.unchanged(indirect, &HashSet::from([name.clone()]))
            {
                continue;
            }
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
    changes.extend(redundant_keys(&curated, &changes, known));
    for change in &changes {
        if let CuratedChange::Redundant { section, name } = change {
            curated[*section]
                .as_object_mut()
                .expect("object")
                .remove(name);
        }
    }
    // A shopping category keyed on a removed or renamed name follows it: the
    // catalog refuses a category for a name that no longer resolves.
    let categories = curated["categories"].as_object_mut().expect("object");
    for change in &changes {
        match change {
            CuratedChange::Remove { name, .. } | CuratedChange::Redundant { name, .. } => {
                categories.remove(name);
            }
            CuratedChange::Rename { from, to, .. } => {
                if let Some(category) = categories.remove(from) {
                    categories.insert(to.clone(), category);
                }
            }
            CuratedChange::Conflict { .. } => {}
        }
    }
    let text = serde_json::to_string_pretty(&Value::Object(curated)).expect("serializes") + "\n";
    (text, changes)
}

/// The names a cleanup must leave resolving as they do: hand-typed
/// shopping-list items, which are matched as typed, every item name the
/// parser produces for the repo's recipe corpora, and the keys prod's stored
/// lines need.
pub struct CleanupNames {
    /// Normalized names matched as written, never re-parsed: hand-typed
    /// shopping-list items and, with `with_prod`, the items prod lines kept
    /// from when they were saved. Their keys are never re-keyed, even when
    /// the parser would split them.
    pub verbatim: BTreeSet<String>,
    /// Item lines (item and note; verbatim items included) whose estimate
    /// removing a redundant key must not change.
    pub corpus: BTreeSet<(String, Option<String>)>,
    /// Keys prod's stored lines need (`PROD_NAMES`), left alone even without
    /// prod's lines at hand.
    pub prod: BTreeSet<String>,
}

/// The curated keys only prod's stored lines need, as last found by
/// `make catalog-clean-aliases PROD_RECIPES=`. Prod lines keep the item parsed
/// at save, so a key the parser no longer produces can still be live there.
pub const PROD_NAMES: &str = "ramekin-core/src/catalog/data/prod_names.json";

impl CleanupNames {
    /// Read the repo's names (`load_repo`) and the committed `PROD_NAMES`.
    pub fn load(root: &Path) -> Self {
        let path = root.join(PROD_NAMES);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        CleanupNames {
            prod: serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("parsing {}: {e}", path.display())),
            ..Self::load_repo(root)
        }
    }

    /// Add prod's stored lines: their items are matched as stored, and their
    /// estimates must not change.
    pub fn with_prod(mut self, lines: impl IntoIterator<Item = (String, Option<String>)>) -> Self {
        for (item, note) in lines {
            if item.trim().is_empty() {
                continue;
            }
            self.verbatim.insert(normalize(&item));
            self.corpus.insert((item, note));
        }
        self
    }

    /// Read the corpora under the repo root: the pipeline and Paprika parser
    /// fixtures, the pipeline snapshots and the shopping-list corpus.
    pub fn load_repo(root: &Path) -> Self {
        let read = |path: &Path| -> Value {
            let text = std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
            serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("parsing {}: {e}", path.display()))
        };
        let lines = |value: &Value, prefix: &str| -> Vec<(String, Option<String>)> {
            value
                .as_array()
                .expect("an array of ingredients")
                .iter()
                .filter_map(|line| {
                    let item = line.pointer(&format!("{prefix}/item"))?.as_str()?;
                    let note = line
                        .pointer(&format!("{prefix}/note"))
                        .and_then(Value::as_str);
                    (!item.trim().is_empty()).then(|| (item.to_string(), note.map(str::to_string)))
                })
                .collect()
        };
        let shopping = lines(&read(&root.join(SHOPPING_CORPUS)), "");
        let verbatim = shopping.iter().map(|(item, _)| normalize(item)).collect();
        let mut corpus: BTreeSet<_> = shopping.into_iter().map(|(item, _)| (item, None)).collect();
        for (dir, prefix) in [
            (
                "ramekin-core/tests/fixtures/ingredient_parsing/pipeline",
                "/expected",
            ),
            (
                "ramekin-core/tests/fixtures/ingredient_parsing/paprika",
                "/expected",
            ),
            ("data/pipeline-snapshots", ""),
        ] {
            let dir = root.join(dir);
            let entries = std::fs::read_dir(&dir)
                .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()));
            for entry in entries {
                let path = entry.expect("a directory entry").path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    corpus.extend(lines(&read(&path)["ingredients"], prefix));
                }
            }
        }
        CleanupNames {
            verbatim,
            corpus,
            prod: BTreeSet::new(),
        }
    }
}

const SHOPPING_CORPUS: &str = "data/shopping-list-categories.json";

/// A word with any plural ending dropped, so a name that reaches a key only
/// through a singular or plural spelling (`plural_variants`) still shares
/// its words.
fn stem(word: &str) -> String {
    let word = match word.strip_suffix("ies") {
        Some(stem) => format!("{stem}y"),
        None => word.to_string(),
    };
    let word = word.strip_suffix('s').unwrap_or(&word);
    word.strip_suffix('e').unwrap_or(word).to_string()
}

fn stems(name: &str) -> BTreeSet<String> {
    name.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(stem)
        .collect()
}

/// The corpus lines that could reach a key, and whether hiding keys changes
/// their estimates. A name reaches a key only through spellings built from
/// its own words, so only lines whose item has all of the key's words count.
struct Reach<'a> {
    corpus: Vec<&'a (String, Option<String>)>,
    by_stem: HashMap<String, Vec<usize>>,
    /// Each checked line's estimate with nothing hidden.
    now: HashMap<(String, Option<String>), String>,
}

impl<'a> Reach<'a> {
    fn new(corpus: &'a BTreeSet<(String, Option<String>)>) -> Self {
        let corpus: Vec<_> = corpus.iter().collect();
        let mut by_stem: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, (item, _)) in corpus.iter().enumerate() {
            for word in stems(&item.to_lowercase()) {
                by_stem.entry(word).or_default().push(index);
            }
        }
        Reach {
            corpus,
            by_stem,
            now: HashMap::new(),
        }
    }

    fn reaching(&self, key: &str) -> Vec<&'a (String, Option<String>)> {
        let mut reaching: Option<BTreeSet<usize>> = None;
        for word in stems(key) {
            let names: BTreeSet<usize> = self
                .by_stem
                .get(&word)
                .into_iter()
                .flatten()
                .copied()
                .collect();
            reaching = Some(match reaching {
                Some(found) => found.intersection(&names).copied().collect(),
                None => names,
            });
        }
        reaching
            .into_iter()
            .flatten()
            .map(|index| self.corpus[index])
            .collect()
    }

    /// Whether every line estimates as it does now with `hidden` hidden.
    fn unchanged(
        &mut self,
        lines: Vec<(String, Option<String>)>,
        hidden: &HashSet<String>,
    ) -> bool {
        lines.into_iter().all(|line| {
            let after = line_outcome(&line, hidden);
            let before = self
                .now
                .entry(line)
                .or_insert_with_key(|line| line_outcome(line, &HashSet::new()));
            after == *before
        })
    }
}

/// The keys left in `curated` that nothing needs: hidden from the catalog
/// (with the keys `changes` already removed), the key itself and every corpus
/// line that could reach it (`Reach`) still estimate as they do now
/// (`line_outcome`). Longer keys go first, so of two keys that only need each
/// other ("tomato", "tomatoes") the shorter stays.
///
/// Ambiguous (null) aliases are deliberate overrides and always stay, as do
/// names bespoke.json gives pieces (it needs the exact name) and keys prod
/// needs (`known.prod`); keys the
/// compiled catalog doesn't have yet are left for the next run.
fn redundant_keys(
    curated: &Map<String, Value>,
    changes: &[CuratedChange],
    known: &CleanupNames,
) -> Vec<CuratedChange> {
    let bespoke: Value = serde_json::from_str(BESPOKE_JSON).expect("bespoke.json is JSON");
    let bespoke = bespoke["pieces"].as_object().expect("bespoke pieces");
    let mut reach = Reach::new(&known.corpus);
    let mut hidden: HashSet<String> = changes
        .iter()
        .filter_map(|change| match change {
            CuratedChange::Remove { name, .. } => Some(name.clone()),
            CuratedChange::Rename { from, .. } => Some(from.clone()),
            _ => None,
        })
        .collect();
    let mut keys: Vec<(&'static str, &String)> = ["aliases", "not_food"]
        .into_iter()
        .flat_map(|section| {
            curated[section]
                .as_object()
                .expect("curated section is an object")
                .iter()
                .filter(|(name, target)| {
                    !target.is_null()
                        && !bespoke.contains_key(*name)
                        && !known.prod.contains(*name)
                        && CATALOG.index.contains_key(*name)
                })
                .map(move |(name, _)| (section, name))
        })
        .collect();
    keys.sort_by_key(|(_, name)| std::cmp::Reverse(name.len()));
    let mut redundant = Vec::new();
    for (section, key) in keys {
        hidden.insert(key.clone());
        let lines = std::iter::once((key.clone(), None))
            .chain(reach.reaching(key).into_iter().cloned())
            .collect();
        if reach.unchanged(lines, &hidden) {
            redundant.push(CuratedChange::Redundant {
                section,
                name: key.clone(),
            });
        } else {
            hidden.remove(key);
        }
    }
    redundant
}
