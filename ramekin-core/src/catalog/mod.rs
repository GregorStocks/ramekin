//! Ingredient catalog: the one place that turns a written ingredient name into
//! a known food.
//!
//! Every food in the pinned USDA SR Legacy release is an entry, and
//! `data/curated.json` adds hand-maintained entries (with cited densities and,
//! for foods no database has, cited calories) and aliases on top. Curated entries may also link foods from the secondary USDA FNDDS
//! release, which are not entries or names on their own. Resolution (which
//! food a name is) is separate from attributes (calories via `fdc_id` or a
//! cited value, density via `grams_per_cup`), so an entry can be recognized
//! even when one attribute is deliberately unknown. See README.md for the data
//! rules.

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
    candidates, is_ambiguous, learned_estimate, learned_key_resolves, resolve_line_with,
    resolve_with, unlearned_name, EstimatedFood, Learned, LearnedTarget,
};
pub use resolve::{chosen_alternative, resolve, Resolution, Via};
pub use volume::{
    is_volume_unit, volume_to_cups, CUPS_PER_FL_OZ, CUPS_PER_GALLON, CUPS_PER_L, CUPS_PER_ML,
    CUPS_PER_PINT, CUPS_PER_QUART, CUPS_PER_TBSP, CUPS_PER_TSP,
};

const USDA_JSON: &str = include_str!("data/usda.json");
const FNDDS_JSON: &str = include_str!("data/fndds.json");
pub const CURATED_JSON: &str = include_str!("data/curated.json");
const BESPOKE_JSON: &str = include_str!("data/bespoke.json");
const RULE_VERSION: &str = "catalog-v5";

/// A food from a pinned USDA release (SR Legacy, or FNDDS for foods SR Legacy
/// lacks).
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

/// A secondary USDA release: foods only, linked from curated entries.
#[derive(Deserialize)]
struct FoodsFile {
    foods: Vec<UsdaFood>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CuratedEntry {
    #[serde(default)]
    kind: Kind,
    fdc_id: Option<u32>,
    /// Cited calories for a food no linked USDA food covers.
    kcal_per_100g: Option<CitedValue>,
    grams_per_cup: Option<CuratedDensity>,
    /// A product's shopping-list category; foods take theirs from the
    /// top-level `categories`.
    category: Option<String>,
    /// Commonly listed without an amount ("pepper"); such lines are negligible.
    #[serde(default)]
    trace_ok: bool,
    /// Pieces this name means, cited, used instead of the linked food's
    /// ("hamburger buns" and "hot dog buns" share one USDA food).
    #[serde(default)]
    portions: BTreeMap<String, CitedValue>,
    /// The piece a bare count of this name means; one of `portions`.
    default_portion: Option<String>,
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

/// A cited piece weight must be positive and keyed like imported pieces
/// ("head", "tortilla medium"), and cite a source with an https:// url.
fn assert_valid_piece(piece: &str, cited: &CitedValue, owner: &str) {
    assert!(
        valid_density(cited.value),
        "invalid weight for {owner:?} piece {piece:?}"
    );
    assert_eq!(
        piece,
        piece_unit(piece),
        "{owner:?} piece {piece:?} must be normalized"
    );
    assert!(
        !cited.source.trim().is_empty() && cited.url.starts_with("https://"),
        "{owner:?} piece {piece:?} needs a source and an https:// url"
    );
}

/// Piece weights no published database gives (a shallot, a bunch of
/// scallions, a standard can), each with the basis for its number.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BespokeFile {
    /// Keyed by any catalog name: the pieces join the entry it resolves to.
    pieces: BTreeMap<String, BespokeFood>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BespokeFood {
    pieces: BTreeMap<String, BespokePiece>,
    /// The piece a bare count means, if the entry has none.
    default: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BespokePiece {
    grams: f64,
    /// Where the number comes from: a reference's statement, or a stated
    /// convention ("standard 15-ounce can").
    basis: String,
    url: Option<String>,
}

/// A hand-curated number and the record it came from.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CitedValue {
    value: f64,
    source: String,
    url: String,
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
    /// Corrections to SR Legacy foods, keyed by their (unique) description.
    food_overrides: BTreeMap<String, FoodOverride>,
    /// Shopping-list categories for foods and ambiguous names, keyed by a
    /// curated entry id, USDA name, or ambiguous name (never an alias of a
    /// food).
    categories: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FoodOverride {
    /// Pieces the release doesn't weigh, cited ("head" of garlic), added to
    /// the food's own; keys are normalized like imported pieces.
    #[serde(default)]
    portions: BTreeMap<String, CitedValue>,
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
    /// The USDA food this entry is, if any.
    pub fdc_id: Option<u32>,
    /// Calories: a curated entry's cited value, else its linked food's.
    pub kcal_per_100g: Option<f64>,
    pub grams_per_cup: Option<f64>,
    /// Shopping-list category, when the catalog knows it.
    pub category: Option<String>,
    /// The linked food has no calories (salt, water, baking soda), so any line
    /// of it is negligible.
    pub zero_calorie: bool,
    /// Commonly listed without an amount (pepper, dried spices); a line of it
    /// with no quantity, or a pinch or dash, is negligible.
    pub trace_ok: bool,
    /// Grams per piece: this name's own cited pieces if it has any, else the
    /// linked food's; plus any bespoke pieces.
    pub portions: BTreeMap<String, f64>,
    /// The piece a bare count means.
    pub default_portion: Option<String>,
}

#[derive(Clone, Copy)]
enum Target {
    Entry(usize),
    /// An index into `Catalog::ambiguous`.
    Ambiguous(usize),
    NotFood,
}

/// A name that could mean several foods ("cheese").
#[derive(Debug)]
pub struct AmbiguousName {
    pub name: String,
    /// Shopping-list category, when curated.
    pub category: Option<&'static str>,
}

struct Catalog {
    entries: Vec<Entry>,
    ambiguous: Vec<AmbiguousName>,
    foods: HashMap<u32, UsdaFood>,
    index: HashMap<String, Target>,
    version: String,
}

/// Lowercase and collapse whitespace; every catalog key is stored this way.
pub fn normalize(value: &str) -> String {
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
    let fndds: FoodsFile = serde_json::from_str(FNDDS_JSON).expect("invalid catalog fndds.json");
    let curated: CuratedFile =
        serde_json::from_str(CURATED_JSON).expect("invalid catalog curated.json");

    // Only SR Legacy foods are entries and names; FNDDS foods are reached
    // through curated entries.
    let sr_food_ids: HashSet<u32> = usda.foods.iter().map(|food| food.fdc_id).collect();
    let mut foods = HashMap::new();
    for food in usda.foods.into_iter().chain(fndds.foods) {
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
        // SR Legacy foods only: FNDDS foods are reached through curated
        // entries, which carry their own corrections.
        let mut matches = foods
            .values_mut()
            .filter(|food| sr_food_ids.contains(&food.fdc_id) && food.description == description);
        let food = matches
            .next()
            .unwrap_or_else(|| panic!("food override {description:?} matches no food"));
        assert!(
            matches.next().is_none(),
            "food override {description:?} matches several foods"
        );
        for (piece, cited) in food_override.portions {
            assert_valid_piece(&piece, &cited, &description);
            assert!(
                food.portions.insert(piece.clone(), cited.value).is_none(),
                "food override {description:?} repeats the food's piece {piece:?}"
            );
        }
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
        let kcal_per_100g = match (&curated_entry.kcal_per_100g, food) {
            (Some(cited), None) => {
                assert!(
                    cited.value.is_finite() && cited.value >= 0.0,
                    "invalid calories for {id:?}"
                );
                assert!(
                    !cited.source.trim().is_empty() && cited.url.starts_with("https://"),
                    "calories for {id:?} need a source and an https:// url"
                );
                Some(cited.value)
            }
            (Some(_), Some(_)) => {
                panic!("curated entry {id:?} cites calories and links a USDA food")
            }
            (None, food) => food.and_then(|food| food.kcal_per_100g),
        };
        let grams_per_cup = match curated_entry.grams_per_cup {
            Some(CuratedDensity::Value { value, .. }) => {
                assert!(valid_density(value), "invalid density for {id:?}");
                Some(value)
            }
            Some(CuratedDensity::Unknown { .. }) => None,
            None => food.and_then(|food| food.grams_per_cup),
        };
        // A name's own cited pieces replace its food's: a baguette's slice is
        // not generic French bread's.
        let (portions, default_portion) = if curated_entry.portions.is_empty() {
            assert!(
                curated_entry.default_portion.is_none(),
                "curated entry {id:?} has a default portion but no portions"
            );
            (
                food.map(|food| food.portions.clone()).unwrap_or_default(),
                food.and_then(|food| food.default_portion.clone()),
            )
        } else {
            let portions: BTreeMap<String, f64> = curated_entry
                .portions
                .iter()
                .map(|(piece, cited)| {
                    assert_valid_piece(piece, cited, &id);
                    (piece.clone(), cited.value)
                })
                .collect();
            (portions, curated_entry.default_portion.clone())
        };
        if let Some(piece) = &default_portion {
            assert!(
                portions.contains_key(piece),
                "curated entry {id:?} default portion {piece:?} is not one of its portions"
            );
        }
        if let Some(category) = &curated_entry.category {
            assert!(
                CATEGORIES.contains(&category.as_str()),
                "curated entry {id:?} has unknown category {category:?}"
            );
        }
        if curated_entry.kind == Kind::Food {
            assert!(
                curated_entry.category.is_none(),
                "food entry {id:?} must take its category from `categories`"
            );
            // A trace-only entry (a spice with no citable calories) is fine:
            // small amounts are negligible and larger ones stay unknown.
            assert!(
                food.is_some()
                    || kcal_per_100g.is_some()
                    || grams_per_cup.is_some()
                    || curated_entry.trace_ok,
                "food entry {id:?} needs an fdc_id, calories, a density, or trace_ok"
            );
        }
        if curated_entry.kind == Kind::Product {
            assert!(
                food.is_none()
                    && kcal_per_100g.is_none()
                    && grams_per_cup.is_none()
                    && curated_entry.category.is_some(),
                "product {id:?} must have a category and no food, calories or density"
            );
        }
        index.insert(id.clone(), Target::Entry(entries.len()));
        entries.push(Entry {
            id,
            kind: curated_entry.kind,
            fdc_id: curated_entry.fdc_id,
            kcal_per_100g,
            grams_per_cup,
            category: curated_entry.category,
            zero_calorie: kcal_per_100g == Some(0.0),
            trace_ok: curated_entry.trace_ok
                || food.is_some_and(|food| is_trace_ok(food, &trace_ok_foods)),
            portions,
            default_portion,
        });
    }

    let mut food_ids: Vec<_> = sr_food_ids.iter().copied().collect();
    food_ids.sort_unstable();
    let mut entry_for_food = HashMap::new();
    for fdc_id in food_ids {
        let food = &foods[&fdc_id];
        entry_for_food.insert(fdc_id, entries.len());
        entries.push(Entry {
            id: food.description.clone(),
            kind: Kind::Food,
            fdc_id: Some(fdc_id),
            kcal_per_100g: food.kcal_per_100g,
            grams_per_cup: food.grams_per_cup,
            category: None,
            zero_calorie: is_zero_calorie(food),
            trace_ok: is_trace_ok(food, &trace_ok_foods),
            portions: food.portions.clone(),
            default_portion: food.default_portion.clone(),
        });
    }

    // Stripped names (the importer resolved their collisions) beat full
    // descriptions, which are ambiguous when several foods share one.
    for (name, fdc_id) in usda.names {
        assert_eq!(name, normalize(&name), "usda names must be normalized");
        let entry = entry_for_food[&fdc_id];
        index.entry(name).or_insert(Target::Entry(entry));
    }
    let sr_foods = || {
        foods
            .values()
            .filter(|food| sr_food_ids.contains(&food.fdc_id))
    };
    let mut ambiguous = Vec::new();
    let mut description_counts: HashMap<&str, usize> = HashMap::new();
    for food in sr_foods() {
        *description_counts.entry(&food.description).or_default() += 1;
    }
    for food in sr_foods() {
        let target = if description_counts[food.description.as_str()] == 1 {
            Target::Entry(entry_for_food[&food.fdc_id])
        } else {
            ambiguous.push(AmbiguousName {
                name: food.description.clone(),
                category: None,
            });
            Target::Ambiguous(ambiguous.len() - 1)
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
            None => {
                ambiguous.push(AmbiguousName {
                    name: alias.clone(),
                    category: None,
                });
                Target::Ambiguous(ambiguous.len() - 1)
            }
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

    let bespoke: BespokeFile =
        serde_json::from_str(BESPOKE_JSON).expect("invalid catalog bespoke.json");
    for (name, extra) in bespoke.pieces {
        assert_eq!(name, normalize(&name), "bespoke names must be normalized");
        let Some(&Target::Entry(entry_index)) = index.get(&name) else {
            panic!("bespoke pieces for {name:?}, which names no single entry");
        };
        let entry = &mut entries[entry_index];
        for (piece, value) in extra.pieces {
            assert_eq!(
                piece,
                piece_unit(&piece),
                "bespoke piece {piece:?} must be normalized"
            );
            assert!(
                valid_density(value.grams),
                "invalid weight for bespoke {name:?} {piece:?}"
            );
            assert!(
                !value.basis.trim().is_empty()
                    && value
                        .url
                        .as_ref()
                        .is_none_or(|url| url.starts_with("https://")),
                "bespoke {name:?} {piece:?} needs a basis (and any url must be https://)"
            );
            // Bespoke numbers never shadow a published one.
            assert!(
                entry.portions.insert(piece.clone(), value.grams).is_none(),
                "bespoke {name:?} {piece:?} shadows a published piece"
            );
        }
        if let Some(piece) = extra.default {
            assert!(
                entry.default_portion.is_none() && entry.portions.contains_key(&piece),
                "bespoke default {piece:?} for {name:?} must be one of its pieces, and the entry must have no default"
            );
            entry.default_portion = Some(piece);
        }
    }

    for (name, category) in curated.categories {
        assert_eq!(name, normalize(&name), "category names must be normalized");
        let category = CATEGORIES
            .into_iter()
            .find(|&c| c == category)
            .unwrap_or_else(|| panic!("{name:?} has unknown category {category:?}"));
        assert!(
            !matches!(curated.aliases.get(&name), Some(Some(_))),
            "category for alias {name:?}: categorize the entry it targets instead"
        );
        match index.get(&name) {
            Some(Target::Entry(entry_index)) => {
                let entry = &mut entries[*entry_index];
                assert!(
                    entry.kind == Kind::Food && entry.category.is_none(),
                    "{name:?} names a product or an already categorized entry"
                );
                entry.category = Some(category.to_string());
            }
            Some(Target::Ambiguous(ambiguous_index)) => {
                let ambiguous_name = &mut ambiguous[*ambiguous_index];
                assert!(
                    ambiguous_name.category.is_none(),
                    "{name:?} names an already categorized ambiguous name"
                );
                ambiguous_name.category = Some(category);
            }
            Some(Target::NotFood) | None => {
                panic!("category for {name:?}, which names no entry or ambiguous name")
            }
        }
    }

    let hash = Sha256::digest(format!(
        "{RULE_VERSION}\n{USDA_JSON}\n{FNDDS_JSON}\n{CURATED_JSON}\n{BESPOKE_JSON}"
    ));
    let hash: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
    Catalog {
        entries,
        ambiguous,
        foods,
        index,
        version: format!("{RULE_VERSION}-sr2018-fndds2024-{hash}"),
    }
});

/// A stable identifier for the catalog data and resolution rules.
pub fn version() -> &'static str {
    &CATALOG.version
}

/// The USDA food (SR Legacy or FNDDS) with this FDC id.
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
///
/// A note naming a milk's beverage form ("carton", "beverage", "refrigerated
/// kind") tries "<item> beverage" first: coconut milk defaults to canned, about
/// six times the calories of the carton drink. Brand names and a bare
/// "refrigerated" don't count, since canned coconut milk is often "refrigerated
/// overnight".
///
/// A note saying the food is peeled ("peeled and cored") swaps a USDA food
/// "with skin" or "with peel" for its peeled counterpart when USDA has one:
/// the skin is discarded whenever the peeling happens. A note giving a fat or
/// lean percentage ("at least 15% fat", "90/10") picks that blend of ground
/// beef, turkey or pork, as does one leading the item ("95% lean ground
/// beef"), which USDA must have exactly or the line stays unresolved.
/// "Drained" and "rinsed" are ignored: the amount is nearly always the can or
/// net weight before draining, which the with-liquid food prices about right.
pub fn resolve_line(item: &str, note: Option<&str>) -> Resolution {
    static COOKED: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?i)^\s*(leftover\s+)?cooked\s*(\(.*\))?\s*$").unwrap()
    });
    static BEVERAGE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?i)\b(cartoned|carton|beverage|refrigerated\s+(kind|variety|type))\b")
            .unwrap()
    });
    // A whole word, so "uncooked" and "precooked ... dry" don't count.
    static COOKED_FOOD: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)\bcooked\b").unwrap());
    if note.is_some_and(|note| BEVERAGE.is_match(note)) {
        if let resolved @ Resolution::Entry { .. } = resolve(&format!("{item} beverage")) {
            return resolved;
        }
    }
    if note.is_some_and(|note| COOKED.is_match(note)) {
        if let resolved @ Resolution::Entry { .. } = resolve(&format!("cooked {item}")) {
            return resolved;
        }
        // The item may already name a cooked food; otherwise the raw food
        // would misstate a cooked measure, so it stays unresolved.
        return match resolve(item) {
            resolved @ Resolution::Entry { entry, .. } if COOKED_FOOD.is_match(&entry.id) => {
                resolved
            }
            _ => Resolution::Unresolved,
        };
    }
    // A blend written before the food ("95% lean ground beef", "80/20 ground
    // beef"). The "%" must follow the first number, so a range doesn't match.
    static BLEND_PREFIX: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r"(?i)^\s*(?:\d{1,2}\s*(?:%|percent)[\s-]*(?:lean|fat)|\d{2}\s*/\s*\d{1,2})\b[\s-]*(.+)$",
        )
        .unwrap()
    });
    let note = note.unwrap_or_default();
    let resolved = resolve(item);
    let Resolution::Entry { entry, .. } = resolved else {
        // Only the blend itself: the food's default blend would misstate it.
        return BLEND_PREFIX
            .captures(item)
            .and_then(|caps| match resolve(&caps[1]) {
                Resolution::Entry { entry, .. } => prepared_form(&entry.id, item, note),
                _ => None,
            })
            .unwrap_or(resolved);
    };
    prepared_form(&entry.id, item, note).unwrap_or(resolved)
}

/// The food entry `id` names, in the state the line describes: peeled, or
/// ground meat of a stated fat percentage. Both read the item as well as the
/// note, since the parser can leave them there ("chopped peeled apples",
/// "ground beef, 90% lean"). See `resolve_line`.
fn prepared_form(id: &str, item: &str, note: &str) -> Option<Resolution> {
    // A whole word, so "unpeeled" doesn't count.
    static PEELED: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)\bpeeled\b").unwrap());
    // "not peeled", "peeled only if desired": the skin may stay on. Checked
    // within the clause naming peeled, so "peeled but not cored" is peeled.
    static MAYBE_UNPEELED: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r"(?i)\b(not|never|no|without|unpeeled|if|optional|optionally)\b|n['’]t\b",
        )
        .unwrap()
    });
    // A clause that only makes the rest optional: "peeled, if desired",
    // "peeled (optional)".
    static OPTIONAL: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r"(?i)^\s*(if\s+(desired|preferred|you\s+(like|prefer|want))|optional(ly)?)\s*$",
        )
        .unwrap()
    });
    // The first group catches a range ("85-93% lean", "85% to 93% lean"),
    // which names no blend.
    static LEAN: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r"(?i)(\d\s*(?:%|percent)?\s*(?:-|–|to)\s*)?\b(\d{1,2})\s*(?:%|percent)[\s-]*lean\b",
        )
        .unwrap()
    });
    static FAT: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r"(?i)(\d\s*(?:%|percent)?\s*(?:-|–|to)\s*)?\b(\d{1,2})\s*(?:%|percent)[\s-]*fat\b",
        )
        .unwrap()
    });
    static RATIO: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"\b(\d{2})\s*/\s*(\d{1,2})\b").unwrap());
    static REJECTED: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?i)\b(not|never|no|avoid|without)\b|n['’]t\b").unwrap()
    });
    static BLEND: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"\d+% lean meat / ?\d+% fat").unwrap());
    // Turkey and pork, which USDA names "turkey, ground, 93% lean, 7% fat"
    // and "pork, ground, 84% lean / 16% fat", and defaults to blendless
    // "turkey, ground, raw" and "pork, fresh, ground, raw".
    static GROUND: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r"^(turkey|pork), (?:fresh, )?ground(?:, \d+% lean(?:,| /) \d+% fat)?(, raw)?$",
        )
        .unwrap()
    });
    // USDA names are also indexed without their "(includes foods for ...)".
    let id = id.split(" (includes ").next().unwrap_or(id);
    // Clauses from an alternative on ("or other ground beef, with around 20%
    // fat") describe that food, not this one.
    // The item and note are cut separately, so "apples, or pears" still
    // reads a "peeled" note.
    fn clauses_of(text: &str) -> impl Iterator<Item = &str> {
        text.split([',', ';', '(', ')'])
            .flat_map(|clause| clause.split(" but "))
            .take_while(|clause| !clause.trim_start().to_lowercase().starts_with("or "))
    }
    let clauses = || clauses_of(item).chain(clauses_of(note));
    let peeled = clauses()
        .any(|clause| PEELED.is_match(clause) && !MAYBE_UNPEELED.is_match(clause))
        && !clauses().any(|clause| OPTIONAL.is_match(clause));
    let swapped: Vec<String> = if peeled {
        [("with skin", "without skin"), ("with peel", "peeled")]
            .into_iter()
            .find(|(unpeeled, _)| id.contains(unpeeled))
            .map(|(unpeeled, peeled)| id.replace(unpeeled, peeled))
            .into_iter()
            .collect()
    } else {
        // "not 95% lean", "avoid 95% lean" rule a blend out.
        let written = clauses()
            .filter(|clause| !REJECTED.is_match(clause))
            .collect::<Vec<_>>()
            .join(";");
        let percent = |re: &regex::Regex| {
            let caps = re.captures(&written)?;
            caps.get(1).is_none().then(|| caps[2].parse::<u32>().ok())?
        };
        let lean = percent(&LEAN)
            .or_else(|| percent(&FAT).map(|fat| 100 - fat))
            .or_else(|| {
                let caps = RATIO.captures(&written)?;
                let (lean, fat) = (caps[1].parse::<u32>().ok()?, caps[2].parse::<u32>().ok()?);
                (lean + fat == 100).then_some(lean)
            });
        let lean = lean?;
        let fat = 100 - lean;
        if BLEND.is_match(id) {
            vec![BLEND
                .replace(id, format!("{lean}% lean meat / {fat}% fat"))
                .into_owned()]
        } else if let Some(caps) = GROUND.captures(id) {
            let (meat, raw) = (&caps[1], caps.get(2).map_or("", |raw| raw.as_str()));
            let mut names: Vec<String> = [",", " /"]
                .map(|sep| format!("{meat}, ground, {lean}% lean{sep} {fat}% fat{raw}"))
                .into();
            // USDA's fat-free turkey is what "99% lean ground turkey" means.
            if meat == "turkey" && lean == 99 {
                names.push(format!("turkey, ground, fat free{raw}"));
            }
            names
        } else {
            Vec::new()
        }
    };
    // Only that exact food: trimming "apples, raw, granny smith, without skin"
    // would land back on unpeeled apples.
    swapped.iter().find_map(|name| match resolve(name) {
        resolved @ Resolution::Entry {
            via: Via::Exact, ..
        } => Some(resolved),
        _ => None,
    })
}

/// Grams per US cup for an ingredient line (see `resolve_line`).
pub fn line_grams_per_cup(item: &str, note: Option<&str>) -> Option<f64> {
    resolution_grams_per_cup(resolve_line(item, note))
}

/// `line_grams_per_cup`, with learned names (see `resolve_line_with`).
pub fn line_grams_per_cup_with(item: &str, note: Option<&str>, learned: &Learned) -> Option<f64> {
    resolution_grams_per_cup(resolve_line_with(item, note, learned))
}

fn resolution_grams_per_cup(resolution: Resolution) -> Option<f64> {
    match resolution {
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
        | Resolution::Ambiguous(_)
        | Resolution::Unresolved => None,
    }
}

/// Whether a written name is not something eaten: a leftover header, a
/// `not_food` phrase, or products only ("parchment paper and aluminum foil").
pub fn is_non_food(item: &str) -> bool {
    resolution_is_non_food(resolve(item))
}

/// `is_non_food`, with learned names (see `resolve_with`).
pub fn is_non_food_with(item: &str, learned: &Learned) -> bool {
    resolution_is_non_food(resolve_with(item, learned))
}

fn resolution_is_non_food(resolution: Resolution) -> bool {
    match resolution {
        Resolution::NotFood => true,
        Resolution::Entry { entry, .. } => entry.kind == Kind::Product,
        Resolution::Compound(entries) => entries.iter().all(|entry| entry.kind == Kind::Product),
        Resolution::Ambiguous(_) | Resolution::Unresolved => false,
    }
}

/// The shopping-list category the catalog assigns a written name, if any. A
/// compound line ("salt and pepper") takes its first food's category, and an
/// ambiguous name ("cheese") its own.
pub fn category(item: &str) -> Option<&'static str> {
    let entry = match resolve(item) {
        Resolution::Entry { entry, .. } => entry,
        Resolution::Compound(entries) => entries[0],
        Resolution::Ambiguous(ambiguous) => return ambiguous.category,
        Resolution::NotFood | Resolution::Unresolved => return None,
    };
    entry.category.as_deref()
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
pub fn piece_unit(unit: &str) -> String {
    let unit = normalize(&unit.replace('-', " "));
    let (first, rest) = unit.split_once(' ').unwrap_or((&unit, ""));
    let first = singular(first);
    if rest.is_empty() {
        first
    } else {
        format!("{first} {rest}")
    }
}

/// "leaves" is "leaf", "cloves" is "clove"; "glass" and "as" stay.
fn singular(word: &str) -> String {
    if let Some(stem) = word.strip_suffix("leaves") {
        format!("{stem}leaf")
    } else if let Some(stem) = word
        .strip_suffix('s')
        .filter(|stem| stem.chars().count() >= 3 && !stem.ends_with('s'))
    {
        stem.to_string()
    } else {
        word.to_string()
    }
}

/// Grams in one piece of this entry's food: `unit` is the counted unit ("clove",
/// "large", "stalks"), or None for a bare count ("3 eggs"). A size the food has
/// no exact portion for uses the one sized piece ("medium" potato is "potato
/// medium"), else the default piece; a piece with sizes uses its medium
/// ("stalk" celery is "stalk medium").
pub fn grams_per_piece(entry: &Entry, unit: Option<&str>) -> Option<f64> {
    let portions = &entry.portions;
    let default = || entry.default_portion.as_ref().map(|piece| portions[piece]);
    let Some(unit) = unit.filter(|unit| !unit.trim().is_empty()) else {
        return default();
    };
    let key = piece_unit(unit);
    if let Some(&grams) = portions.get(&key) {
        return Some(grams);
    }
    // A sized piece ("small head") uses the piece at that size ("head
    // small"); with no such portion ("large lemon") it is the piece, and a
    // many-word piece counts in the plural too ("blood oranges").
    let (size, unsized_piece) = SIZES
        .iter()
        .find_map(|size| Some((Some(*size), key.strip_prefix(size)?.strip_prefix(' ')?)))
        .unwrap_or((None, &key));
    let piece = piece_unit(unsized_piece);
    let singular_piece = match piece.rsplit_once(' ') {
        Some((head, last)) => format!("{head} {}", singular(last)),
        None => piece.clone(),
    };
    if let Some(&grams) = size
        .and_then(|size| portions.get(&format!("{piece} {size}")))
        .or_else(|| portions.get(&piece))
        .or_else(|| portions.get(&singular_piece))
    {
        return Some(grams);
    }
    if SIZES.contains(&key.as_str()) {
        let suffix = format!(" {key}");
        let mut sized = portions.iter().filter(|(piece, _)| {
            piece.ends_with(&suffix)
                && !PARTIAL_PIECES.contains(&piece.split(' ').next().unwrap_or_default())
        });
        return match (sized.next(), sized.next()) {
            (Some((_, &grams)), None) => Some(grams),
            (None, _) => default(),
            (Some(_), Some(_)) => None,
        };
    }
    portions.get(&format!("{key} medium")).copied()
}
