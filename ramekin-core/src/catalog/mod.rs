//! Ingredient catalog: the one place that turns a written ingredient name into
//! a known food.
//!
//! Every food in the pinned USDA SR Legacy release is an entry, and
//! `data/curated.json` adds hand-maintained entries (with cited densities),
//! aliases, and display rewrites on top. Resolution (which food a name is) is
//! separate from attributes (calories via `fdc_id`, density via
//! `grams_per_cup`), so an entry can be recognized even when one attribute is
//! deliberately unknown. See README.md for the data rules.

mod cleanup;
mod learned;
mod resolve;
mod volume;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::LazyLock;

use serde::Deserialize;

use crate::ingredient_categorizer::CATEGORIES;
use sha2::{Digest, Sha256};

pub use cleanup::{clean_curated, parsed_name, CuratedChange};
pub use learned::{
    candidates, resolve_line_with, resolve_with, unlearned_name, Learned, LearnedTarget,
};
pub use resolve::{resolve, Resolution, Via};
pub use volume::{
    is_volume_unit, volume_to_cups, CUPS_PER_FL_OZ, CUPS_PER_GALLON, CUPS_PER_L, CUPS_PER_ML,
    CUPS_PER_PINT, CUPS_PER_QUART, CUPS_PER_TBSP, CUPS_PER_TSP,
};

const USDA_JSON: &str = include_str!("data/usda.json");
pub const CURATED_JSON: &str = include_str!("data/curated.json");
const RULE_VERSION: &str = "catalog-v2";

/// A food from the pinned USDA SR Legacy release.
#[derive(Debug, Deserialize)]
pub struct UsdaFood {
    pub fdc_id: u32,
    pub description: String,
    pub kcal_per_100g: Option<f64>,
    pub grams_per_cup: Option<f64>,
    /// Grams per piece, keyed by piece name ("clove", "large", "stalk medium").
    pub portions: BTreeMap<String, f64>,
    /// The piece a bare count ("3 carrots") means, if any.
    pub default_portion: Option<String>,
}

#[derive(Deserialize)]
struct UsdaFile {
    foods: Vec<UsdaFood>,
    names: BTreeMap<String, u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CuratedEntry {
    #[serde(default)]
    kind: Kind,
    fdc_id: Option<u32>,
    grams_per_cup: Option<CuratedDensity>,
    /// Shopping-list category; overrides the keyword categorizer.
    category: Option<String>,
    /// Commonly listed without an amount ("pepper"); such lines are negligible.
    #[serde(default)]
    trace_ok: bool,
}

/// Whether an entry is something you eat or a purchasable non-food.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    Food,
    /// Bought at the store but not eaten (parchment paper, skewers). Carries
    /// only a shopping category; calorie estimates skip it.
    Product,
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
    /// Names that are not ingredients at all ("to serve"), with the reason.
    not_food: BTreeMap<String, String>,
    rewrites: BTreeMap<String, String>,
    /// Corrections to USDA foods, keyed by their (unique) description.
    food_overrides: BTreeMap<String, FoodOverride>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FoodOverride {
    /// Replaces the imported default piece ("3 eggs" means large eggs).
    default_portion: Option<String>,
    /// Marks a food trace_ok (fresh herbs are listed by the sprig).
    #[serde(default)]
    trace_ok: bool,
}

/// A catalog entry a name can resolve to.
#[derive(Debug)]
pub struct Entry {
    /// A curated entry's name, or a USDA food's lowercased description.
    pub id: String,
    pub kind: Kind,
    /// The USDA food supplying calories, if known.
    pub fdc_id: Option<u32>,
    pub grams_per_cup: Option<f64>,
    /// Shopping-list category, when the catalog knows it.
    pub category: Option<String>,
    /// The linked food has no calories (salt, water, baking soda), so any line
    /// of it is negligible.
    pub zero_calorie: bool,
    /// Commonly listed without an amount (pepper, dried spices); a line of it
    /// with no quantity, or a pinch or dash, is negligible.
    pub trace_ok: bool,
}

#[derive(Clone, Copy)]
enum Target {
    Entry(usize),
    Ambiguous,
    NotFood,
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

/// Foods with no calories at all, so any amount is negligible.
fn is_zero_calorie(food: &UsdaFood) -> bool {
    food.kcal_per_100g == Some(0.0)
}

/// Dried spices (USDA "spices, …", including black pepper) and curated foods
/// are commonly listed without an amount.
fn is_trace_ok(food: &UsdaFood, trace_ok_foods: &HashSet<u32>) -> bool {
    food.description.starts_with("spices, ") || trace_ok_foods.contains(&food.fdc_id)
}

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    let usda: UsdaFile = serde_json::from_str(USDA_JSON).expect("invalid catalog usda.json");
    let curated: CuratedFile =
        serde_json::from_str(CURATED_JSON).expect("invalid catalog curated.json");

    let mut foods = HashMap::new();
    for food in usda.foods {
        assert!(food.kcal_per_100g.is_none_or(|k| k.is_finite() && k >= 0.0));
        assert!(food.grams_per_cup.is_none_or(valid_density));
        assert!(food.portions.values().all(|&grams| valid_density(grams)));
        assert!(food
            .default_portion
            .as_ref()
            .is_none_or(|piece| food.portions.contains_key(piece)));
        assert!(
            foods.insert(food.fdc_id, food).is_none(),
            "duplicate fdc_id"
        );
    }

    let mut trace_ok_foods = HashSet::new();
    for (description, food_override) in curated.food_overrides {
        let mut matches = foods
            .values_mut()
            .filter(|food| food.description == description);
        let food = matches
            .next()
            .unwrap_or_else(|| panic!("food override {description:?} matches no food"));
        assert!(
            matches.next().is_none(),
            "food override {description:?} matches several foods"
        );
        if let Some(piece) = food_override.default_portion {
            assert!(
                food.portions.contains_key(&piece),
                "food override {description:?} has unknown portion {piece:?}"
            );
            food.default_portion = Some(piece);
        }
        if food_override.trace_ok {
            trace_ok_foods.insert(food.fdc_id);
        }
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
        if let Some(category) = &curated_entry.category {
            assert!(
                CATEGORIES.contains(&category.as_str()),
                "curated entry {id:?} has unknown category {category:?}"
            );
        }
        if curated_entry.kind == Kind::Product {
            assert!(
                food.is_none() && grams_per_cup.is_none() && curated_entry.category.is_some(),
                "product {id:?} must have a category and no food or density"
            );
        }
        index.insert(id.clone(), Target::Entry(entries.len()));
        entries.push(Entry {
            id,
            kind: curated_entry.kind,
            fdc_id: curated_entry.fdc_id,
            grams_per_cup,
            category: curated_entry.category,
            zero_calorie: food.is_some_and(is_zero_calorie),
            trace_ok: curated_entry.trace_ok
                || food.is_some_and(|food| is_trace_ok(food, &trace_ok_foods)),
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
            kind: Kind::Food,
            fdc_id: Some(fdc_id),
            grams_per_cup: food.grams_per_cup,
            category: None,
            zero_calorie: is_zero_calorie(food),
            trace_ok: is_trace_ok(food, &trace_ok_foods),
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

    for name in curated.not_food.keys() {
        assert_eq!(*name, normalize(name), "not_food names must be normalized");
        assert!(
            index.insert(name.clone(), Target::NotFood).is_none(),
            "not_food name {name:?} shadows a catalog name"
        );
    }

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

/// Resolve an ingredient line, using its note when it changes the food. The
/// parser keeps "cooked" in the note ("brown rice, cooked"), and cooked grains
/// differ from dry ones about threefold. Only a note that is exactly "cooked"
/// (or "leftover cooked"), optionally followed by a parenthetical clarifier
/// ("cooked (about 1 cup uncooked)"), states the measured food is cooked and tries "cooked
/// <item>" first. Anything longer ("cooked and crumbled", "cooked, drained,
/// and cut") is a cooking instruction for a raw or dry measure.
pub fn resolve_line(item: &str, note: Option<&str>) -> Resolution {
    static COOKED: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?i)^\s*(leftover\s+)?cooked\s*(\(.*\))?\s*$").unwrap()
    });
    if note.is_some_and(|note| COOKED.is_match(note)) {
        if let resolved @ Resolution::Entry { .. } = resolve(&format!("cooked {item}")) {
            return resolved;
        }
        // The item may already name a cooked food; otherwise the raw food
        // would misstate a cooked measure, so it stays unresolved.
        return match resolve(item) {
            resolved @ Resolution::Entry { entry, .. } if entry.id.contains("cooked") => resolved,
            _ => Resolution::Unresolved,
        };
    }
    resolve(item)
}

/// Grams per US cup for an ingredient line (see `resolve_line`).
pub fn line_grams_per_cup(item: &str, note: Option<&str>) -> Option<f64> {
    match resolve_line(item, note) {
        Resolution::Entry { entry, .. } => entry.grams_per_cup,
        _ => None,
    }
}

/// Grams per US cup for a written ingredient name, if its entry has a density.
/// A compound line has none: the ratio of its foods is unknown.
pub fn grams_per_cup(item: &str) -> Option<f64> {
    match resolve(item) {
        Resolution::Entry { entry, .. } => entry.grams_per_cup,
        Resolution::Compound(_)
        | Resolution::NotFood
        | Resolution::Ambiguous
        | Resolution::Unresolved => None,
    }
}

/// Whether a written name is not something eaten: a leftover header, a
/// `not_food` phrase, or products only ("parchment paper and aluminum foil").
pub fn is_non_food(item: &str) -> bool {
    match resolve(item) {
        Resolution::NotFood => true,
        Resolution::Entry { entry, .. } => entry.kind == Kind::Product,
        Resolution::Compound(entries) => entries.iter().all(|entry| entry.kind == Kind::Product),
        Resolution::Ambiguous | Resolution::Unresolved => false,
    }
}

/// The shopping-list category the catalog assigns a written name, if any. A
/// compound line ("salt and pepper") takes its first food's category.
pub fn category(item: &str) -> Option<&'static str> {
    let entry = match resolve(item) {
        Resolution::Entry { entry, .. } => entry,
        Resolution::Compound(entries) => entries[0],
        Resolution::NotFood | Resolution::Ambiguous | Resolution::Unresolved => return None,
    };
    entry.category.as_deref()
}

/// The display rewrite for an ingredient name (e.g. "salt" -> "kosher salt").
pub fn rewrite(item: &str) -> Option<&'static str> {
    CATALOG.rewrites.get(&normalize(item)).map(String::as_str)
}

/// Size words a count can carry ("2 large eggs").
const SIZES: [&str; 5] = ["small", "medium", "large", "extra large", "jumbo"];

/// Parts of a piece; a size alone never means one of these. Matches the
/// importer's `PARTIAL_PIECES`.
const PARTIAL_PIECES: [&str; 8] = [
    "slice", "strip", "wedge", "ring", "cube", "chip", "pat", "spear",
];

/// Normalize a counted unit to a portion key: lowercase, "extra-large" ->
/// "extra large", and the first word singular ("cloves" -> "clove"). Matches
/// the importer's `portion_key`.
fn piece_unit(unit: &str) -> String {
    let unit = normalize(&unit.replace('-', " "));
    let (first, rest) = unit.split_once(' ').unwrap_or((&unit, ""));
    let first = if let Some(stem) = first.strip_suffix("leaves") {
        format!("{stem}leaf")
    } else if let Some(stem) = first
        .strip_suffix('s')
        .filter(|stem| stem.chars().count() >= 3 && !stem.ends_with('s'))
    {
        stem.to_string()
    } else {
        first.to_string()
    };
    if rest.is_empty() {
        first
    } else {
        format!("{first} {rest}")
    }
}

/// Grams in one piece of this entry's food: `unit` is the counted unit ("clove",
/// "large", "stalks"), or None for a bare count ("3 eggs"). A size the food has
/// no exact portion for uses the one sized piece ("medium" potato is "potato
/// medium"), else the default piece; a piece with sizes uses its medium
/// ("stalk" celery is "stalk medium").
pub fn grams_per_piece(entry: &Entry, unit: Option<&str>) -> Option<f64> {
    let food = food(entry.fdc_id?)?;
    let default = || {
        food.default_portion
            .as_ref()
            .map(|piece| food.portions[piece])
    };
    let Some(unit) = unit.filter(|unit| !unit.trim().is_empty()) else {
        return default();
    };
    let key = piece_unit(unit);
    if let Some(&grams) = food.portions.get(&key) {
        return Some(grams);
    }
    if SIZES.contains(&key.as_str()) {
        let suffix = format!(" {key}");
        let mut sized = food.portions.iter().filter(|(piece, _)| {
            piece.ends_with(&suffix)
                && !PARTIAL_PIECES.contains(&piece.split(' ').next().unwrap_or_default())
        });
        return match (sized.next(), sized.next()) {
            (Some((_, &grams)), None) => Some(grams),
            (None, _) => default(),
            (Some(_), Some(_)) => None,
        };
    }
    food.portions.get(&format!("{key} medium")).copied()
}
