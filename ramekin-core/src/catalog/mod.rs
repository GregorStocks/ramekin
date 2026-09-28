//! Ingredient catalog: the one place that turns a written ingredient name into
//! a known food.
//!
//! Every food in the pinned USDA SR Legacy release is an entry, and
//! `data/curated.json` adds hand-maintained entries (with cited densities),
//! aliases, and display rewrites on top. Resolution (which food a name is) is
//! separate from attributes (calories via `fdc_id`, density via
//! `grams_per_cup`), so an entry can be recognized even when one attribute is
//! deliberately unknown. See README.md for the data rules.

mod resolve;
mod volume;

use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

use serde::Deserialize;
use sha2::{Digest, Sha256};

pub use resolve::{resolve, Resolution, Via};
pub use volume::{
    is_volume_unit, volume_to_cups, CUPS_PER_FL_OZ, CUPS_PER_GALLON, CUPS_PER_L, CUPS_PER_ML,
    CUPS_PER_PINT, CUPS_PER_QUART, CUPS_PER_TBSP, CUPS_PER_TSP,
};

const USDA_JSON: &str = include_str!("data/usda.json");
const CURATED_JSON: &str = include_str!("data/curated.json");
const RULE_VERSION: &str = "catalog-v1";

/// A food from the pinned USDA SR Legacy release.
#[derive(Debug, Deserialize)]
pub struct UsdaFood {
    pub fdc_id: u32,
    pub description: String,
    pub kcal_per_100g: Option<f64>,
    pub grams_per_cup: Option<f64>,
}

#[derive(Deserialize)]
struct UsdaFile {
    foods: Vec<UsdaFood>,
    names: BTreeMap<String, u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CuratedEntry {
    fdc_id: Option<u32>,
    grams_per_cup: Option<CuratedDensity>,
}

/// A curated density overrides the linked USDA food's density, or marks it
/// deliberately unknown.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum CuratedDensity {
    Value {
        value: f64,
        #[allow(dead_code)]
        source: String,
        #[allow(dead_code)]
        url: Option<String>,
    },
    Unknown {
        #[allow(dead_code)]
        none: String,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CuratedFile {
    entries: BTreeMap<String, CuratedEntry>,
    /// A null target marks a name as ambiguous for every attribute.
    aliases: BTreeMap<String, Option<String>>,
    rewrites: BTreeMap<String, String>,
}

/// A catalog entry a name can resolve to.
#[derive(Debug)]
pub struct Entry {
    /// A curated entry's name, or a USDA food's lowercased description.
    pub id: String,
    /// The USDA food supplying calories, if known.
    pub fdc_id: Option<u32>,
    pub grams_per_cup: Option<f64>,
}

#[derive(Clone, Copy)]
enum Target {
    Entry(usize),
    Ambiguous,
}

struct Catalog {
    entries: Vec<Entry>,
    foods: HashMap<u32, UsdaFood>,
    index: HashMap<String, Target>,
    rewrites: HashMap<String, String>,
    version: String,
}

/// Lowercase and collapse whitespace; every catalog key is stored this way.
pub(crate) fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn valid_density(grams_per_cup: f64) -> bool {
    grams_per_cup.is_finite() && grams_per_cup > 0.0
}

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    let usda: UsdaFile = serde_json::from_str(USDA_JSON).expect("invalid catalog usda.json");
    let curated: CuratedFile =
        serde_json::from_str(CURATED_JSON).expect("invalid catalog curated.json");

    let mut foods = HashMap::new();
    for food in usda.foods {
        assert!(food.kcal_per_100g.is_none_or(|k| k.is_finite() && k >= 0.0));
        assert!(food.grams_per_cup.is_none_or(valid_density));
        assert!(
            foods.insert(food.fdc_id, food).is_none(),
            "duplicate fdc_id"
        );
    }

    let mut entries = Vec::new();
    let mut index = HashMap::new();

    // Curated entries come first so they shadow USDA names they share.
    for (id, curated_entry) in curated.entries {
        assert_eq!(id, normalize(&id), "curated entry ids must be normalized");
        let food = curated_entry.fdc_id.map(|fdc_id| {
            foods
                .get(&fdc_id)
                .unwrap_or_else(|| panic!("curated entry {id:?} links unknown fdc_id {fdc_id}"))
        });
        let grams_per_cup = match curated_entry.grams_per_cup {
            Some(CuratedDensity::Value { value, .. }) => {
                assert!(valid_density(value), "invalid density for {id:?}");
                Some(value)
            }
            Some(CuratedDensity::Unknown { .. }) => None,
            None => food.and_then(|food| food.grams_per_cup),
        };
        index.insert(id.clone(), Target::Entry(entries.len()));
        entries.push(Entry {
            id,
            fdc_id: curated_entry.fdc_id,
            grams_per_cup,
        });
    }

    let mut food_ids: Vec<_> = foods.keys().copied().collect();
    food_ids.sort_unstable();
    let mut entry_for_food = HashMap::new();
    for fdc_id in food_ids {
        let food = &foods[&fdc_id];
        entry_for_food.insert(fdc_id, entries.len());
        entries.push(Entry {
            id: food.description.clone(),
            fdc_id: Some(fdc_id),
            grams_per_cup: food.grams_per_cup,
        });
    }

    // Stripped names (the importer resolved their collisions) beat full
    // descriptions, which are ambiguous when several foods share one.
    for (name, fdc_id) in usda.names {
        assert_eq!(name, normalize(&name), "usda names must be normalized");
        let entry = entry_for_food[&fdc_id];
        index.entry(name).or_insert(Target::Entry(entry));
    }
    let mut description_counts: HashMap<&str, usize> = HashMap::new();
    for food in foods.values() {
        *description_counts.entry(&food.description).or_default() += 1;
    }
    for food in foods.values() {
        let target = if description_counts[food.description.as_str()] == 1 {
            Target::Entry(entry_for_food[&food.fdc_id])
        } else {
            Target::Ambiguous
        };
        index.entry(food.description.clone()).or_insert(target);
    }

    // Aliases may only add names, and only point at non-alias names.
    let mut alias_targets = Vec::new();
    for (alias, target) in &curated.aliases {
        assert_eq!(*alias, normalize(alias), "aliases must be normalized");
        assert!(
            !index.contains_key(alias),
            "alias {alias:?} shadows a catalog name"
        );
        let resolved = match target {
            None => Target::Ambiguous,
            Some(target) => match index.get(target) {
                Some(Target::Entry(entry)) => Target::Entry(*entry),
                _ => panic!("alias {alias:?} targets unknown or ambiguous name {target:?}"),
            },
        };
        alias_targets.push((alias.clone(), resolved));
    }
    index.extend(alias_targets);

    for (from, to) in &curated.rewrites {
        assert_eq!(*from, normalize(from), "rewrite sources must be normalized");
        assert!(
            matches!(index.get(&normalize(to)), Some(Target::Entry(_))),
            "rewrite {from:?} -> {to:?} must resolve"
        );
    }

    let hash = Sha256::digest(format!("{RULE_VERSION}\n{USDA_JSON}\n{CURATED_JSON}"));
    let hash: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
    Catalog {
        entries,
        foods,
        index,
        rewrites: curated.rewrites.into_iter().collect(),
        version: format!("{RULE_VERSION}-sr2018-{hash}"),
    }
});

/// A stable identifier for the catalog data and resolution rules.
pub fn version() -> &'static str {
    &CATALOG.version
}

/// The USDA food with this FDC id.
pub fn food(fdc_id: u32) -> Option<&'static UsdaFood> {
    CATALOG.foods.get(&fdc_id)
}

/// Grams per US cup for a written ingredient name, if its entry has a density.
pub fn grams_per_cup(item: &str) -> Option<f64> {
    match resolve(item) {
        Resolution::Entry { entry, .. } => entry.grams_per_cup,
        Resolution::Ambiguous | Resolution::Unresolved => None,
    }
}

/// The display rewrite for an ingredient name (e.g. "salt" -> "kosher salt").
pub fn rewrite(item: &str) -> Option<&'static str> {
    CATALOG.rewrites.get(&normalize(item)).map(String::as_str)
}
