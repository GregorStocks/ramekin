//! Deterministic estimates from a pinned nutrition snapshot. Unknown is never zero.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::ingredient_parser::{Measurement, ParsedIngredient};
use crate::metric_weights::parse_amount;

const DATA: &str = include_str!("data.json");
const ALIASES: &str = include_str!("aliases.json");
const RULE_VERSION: &str = "calories-v3";

#[derive(Deserialize)]
struct Food {
    fdc_id: u32,
    name: String,
    kcal_per_100g: f64,
    grams_per_cup: Option<f64>,
}

struct Database {
    foods: BTreeMap<u32, Food>,
    names: BTreeMap<String, Vec<u32>>,
    aliases: BTreeMap<String, Option<u32>>,
    version: String,
}

static DATABASE: LazyLock<Database> = LazyLock::new(|| {
    #[derive(Deserialize)]
    struct Snapshot {
        foods: Vec<Food>,
    }
    let snapshot: Snapshot = serde_json::from_str(DATA).expect("invalid nutrition snapshot");
    let aliases: BTreeMap<String, Option<u32>> =
        serde_json::from_str(ALIASES).expect("invalid nutrition aliases");
    let mut foods = BTreeMap::new();
    let mut names: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for food in snapshot.foods {
        assert!(food.kcal_per_100g.is_finite() && food.kcal_per_100g >= 0.0);
        assert!(food.grams_per_cup.is_none_or(|g| g.is_finite() && g > 0.0));
        names
            .entry(normalize(&food.name))
            .or_default()
            .push(food.fdc_id);
        assert!(foods.insert(food.fdc_id, food).is_none());
    }
    for (name, id) in &aliases {
        assert_eq!(*name, normalize(name));
        assert!(id.is_none_or(|id| foods.contains_key(&id)));
        assert!(!names.contains_key(name), "alias shadows a USDA food name");
    }
    let hash = Sha256::digest(format!("{RULE_VERSION}\n{DATA}\n{ALIASES}"));
    let hash = hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Database {
        foods,
        names,
        aliases,
        version: format!("{RULE_VERSION}-sr2018-{hash}"),
    }
});

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalorieRange {
    pub min: f64,
    pub max: f64,
}

#[derive(Debug)]
pub struct UnknownIngredient {
    pub index: usize,
    pub item: String,
    pub reason: String,
}

#[derive(Debug)]
pub struct Estimate {
    pub database_version: String,
    pub known_calories: Option<CalorieRange>,
    pub per_serving_calories: Option<CalorieRange>,
    pub unknown_ingredients: Vec<UnknownIngredient>,
    pub summary: String,
    pub per_serving_summary: Option<String>,
}

fn match_food(name: &str) -> Result<&'static Food, &'static str> {
    let name = normalize(name);
    let id = if let Some(alias) = DATABASE.aliases.get(&name) {
        alias.ok_or("Ambiguous ingredient")?
    } else {
        let ids = DATABASE
            .names
            .get(&name)
            .ok_or("No supported nutrition match")?;
        if ids.len() != 1 {
            return Err("Ambiguous ingredient");
        }
        ids[0]
    };
    Ok(&DATABASE.foods[&id])
}

/// Strict quantity grammar: decimals, fractions, mixed numbers and bounded ranges.
/// This does not alter the extraction pipeline's interpretation of ingredients.
fn quantity(value: &str) -> Option<CalorieRange> {
    static GROUPED: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[1-9][0-9]{0,2},[0-9]{3}$").unwrap());
    static MIXED: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^([0-9]+)-([0-9]+/[0-9]+)$").unwrap());
    static NUMBER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+|[0-9]+/[0-9]+|[0-9]+ [0-9]+/[0-9]+)$")
            .unwrap()
    });
    let mut value = value.trim().to_string();
    for (fraction, replacement) in [
        ('¼', "1/4"),
        ('½', "1/2"),
        ('¾', "3/4"),
        ('⅓', "1/3"),
        ('⅔', "2/3"),
        ('⅕', "1/5"),
        ('⅖', "2/5"),
        ('⅗', "3/5"),
        ('⅘', "4/5"),
        ('⅙', "1/6"),
        ('⅚', "5/6"),
        ('⅛', "1/8"),
        ('⅜', "3/8"),
        ('⅝', "5/8"),
        ('⅞', "7/8"),
    ] {
        value = value.replace(fraction, &format!(" {replacement}"));
    }
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let number = |s: &str| {
        let s = MIXED.replace(s.trim(), "$1 $2");
        if GROUPED.is_match(&s) {
            return None;
        }
        let s = s.replace(',', ".");
        if !NUMBER.is_match(&s) {
            return None;
        }
        parse_amount(&s).filter(|n| n.is_finite() && *n >= 0.0 && *n <= 1e12)
    };
    if let Some(amount) = number(&value) {
        return Some(CalorieRange {
            min: amount,
            max: amount,
        });
    }
    for delimiter in [" to ", " or ", "–", "—", "-"] {
        for (index, _) in value.match_indices(delimiter) {
            if let (Some(min), Some(max)) = (
                number(
                    value
                        .get(..index)
                        .expect("regex match starts at a character boundary"),
                ),
                number(
                    value
                        .get(index + delimiter.len()..)
                        .expect("regex match ends at a character boundary"),
                ),
            ) {
                if min <= max {
                    return Some(CalorieRange { min, max });
                }
            }
        }
    }
    None
}

fn grams_per_unit(unit: &str, food: &Food) -> Result<f64, &'static str> {
    let normalized = normalize(unit);
    let unit = match normalized.as_str() {
        "gram" | "grams" | "g" => return Ok(1.0),
        "kilogram" | "kilograms" | "kg" => return Ok(1000.0),
        "milligram" | "milligrams" | "mg" => return Ok(0.001),
        "ounce" | "ounces" | "oz" => return Ok(28.349523125),
        "pound" | "pounds" | "lb" | "lbs" => return Ok(453.59237),
        "cups" => "cup",
        "pints" => "pint",
        "quarts" => "quart",
        "gallons" => "gallon",
        "tablespoon" | "tablespoons" => "tbsp",
        "teaspoon" | "teaspoons" => "tsp",
        "fluid ounce" | "fluid ounces" | "fl oz" => "fl oz",
        "milliliter" | "milliliters" => "ml",
        "liter" | "liters" | "litre" | "litres" => "l",
        other => other,
    };
    let cups = ingredient_density::volume_to_cups(1.0, unit).ok_or("Unsupported quantity unit")?;
    Ok(cups * food.grams_per_cup.ok_or("Missing density for this food")?)
}

fn measurement_grams(
    amount: &str,
    unit: Option<&str>,
    food: &Food,
) -> Result<CalorieRange, &'static str> {
    static PLUS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+(?:plus|\+)\s+").unwrap());
    static EMBEDDED: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(.+?)\s+([a-z][a-z ]*)$").unwrap());
    let amount = normalize(amount);
    let mut total = CalorieRange { min: 0.0, max: 0.0 };
    for segment in PLUS.split(&amount) {
        let (range, factor) = if let Some(range) = quantity(segment) {
            (range, grams_per_unit(unit.unwrap_or(""), food)?)
        } else {
            // Compound measurements embed their units in amount, with no outer
            // unit. A shared outer unit instead applies to every numeric term.
            if unit.is_some_and(|unit| !unit.is_empty()) {
                return Err("Unsupported or missing quantity");
            }
            let parts = EMBEDDED
                .captures(segment)
                .ok_or("Unsupported or missing quantity")?;
            let range = quantity(&parts[1]).ok_or("Unsupported or missing quantity")?;
            (range, grams_per_unit(&parts[2], food)?)
        };
        total.min += range.min * factor;
        total.max += range.max * factor;
    }
    Ok(total)
}

fn contribution(ingredient: &ParsedIngredient) -> Result<CalorieRange, &'static str> {
    let food = match_food(&ingredient.item)?;
    let mut reason = "Missing quantity";
    // Measurements are alternatives, not additive. Prefer the primary whenever
    // supported, then the first supported alternative, so rounded gram enrichments
    // do not replace precise primary amounts.
    for Measurement { amount, unit } in &ingredient.measurements {
        let Some(amount) = amount.as_deref() else {
            reason = "Unsupported or missing quantity";
            continue;
        };
        let grams = match measurement_grams(amount, unit.as_deref(), food) {
            Ok(grams) => grams,
            Err(error) => {
                reason = error;
                continue;
            }
        };
        let factor = food.kcal_per_100g / 100.0;
        return Ok(CalorieRange {
            min: grams.min * factor,
            max: grams.max * factor,
        });
    }
    Err(reason)
}

fn range_text(range: CalorieRange) -> String {
    if range.min == range.max {
        format!("{:.0}", range.min.round())
    } else {
        format!("{:.0}–{:.0}", range.min.floor(), range.max.ceil())
    }
}

fn serving_count(servings: &str) -> Option<f64> {
    static PREFIX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(?:serves(?:\s*:\s*|\s+)|servings?\s*:\s*)").unwrap());
    let text = normalize(servings);
    let text = PREFIX.replace(&text, "");
    let count = text
        .strip_suffix(" servings")
        .or_else(|| text.strip_suffix(" serving"))
        .unwrap_or(&text);
    quantity(count)
        .filter(|r| r.min == r.max && r.min > 0.0)
        .map(|r| r.min)
}

/// All arithmetic and presentation are computed here; clients render the result.
pub fn estimate(
    ingredients: &[ParsedIngredient],
    servings: Option<&str>,
    scale: f64,
) -> Result<Estimate, &'static str> {
    if !scale.is_finite() || scale <= 0.0 || scale > 1e6 {
        return Err("Scale must be finite, greater than zero, and at most 1000000");
    }
    let mut known = None;
    let mut unknown_ingredients = Vec::new();
    for (index, ingredient) in ingredients.iter().enumerate() {
        match contribution(ingredient) {
            Ok(value) => {
                let total = known.get_or_insert(CalorieRange { min: 0.0, max: 0.0 });
                total.min += value.min;
                total.max += value.max;
                if !total.min.is_finite() || !total.max.is_finite() || total.max * scale > 1e15 {
                    return Err("Calorie estimate exceeds supported numeric bounds");
                }
            }
            Err(reason) => unknown_ingredients.push(UnknownIngredient {
                index,
                item: ingredient.item.clone(),
                reason: reason.to_string(),
            }),
        }
    }
    // Divide the original subtotal directly: scaling ingredients and servings
    // cancels out, including at very small scales where a scaled subtotal underflows.
    let count = servings.and_then(serving_count);
    let per_serving = known.zip(count).map(|(total, count)| CalorieRange {
        min: total.min / count,
        max: total.max / count,
    });
    if per_serving.is_some_and(|range| !range.max.is_finite() || range.max > 1e15) {
        return Err("Per-serving estimate exceeds supported numeric bounds");
    }
    let known = known.map(|total| CalorieRange {
        min: total.min * scale,
        max: total.max * scale,
    });
    let unknown_names = unknown_ingredients
        .iter()
        .map(|i| i.item.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let summary = match known {
        Some(range) if unknown_ingredients.is_empty() => format!("Whole recipe: approximately {} calories.", range_text(range)),
        Some(range) => format!("Known ingredients: {} calories, plus unknown calories from {unknown_names}. This is a partial whole-recipe subtotal.", range_text(range)),
        None if ingredients.is_empty() => "No ingredients to estimate.".to_string(),
        None => format!("Whole-recipe calories unknown: {unknown_names}."),
    };
    let per_serving_summary = per_serving.map(|range| {
        if unknown_ingredients.is_empty() {
            format!("Per serving: approximately {} calories.", range_text(range))
        } else {
            format!("Known ingredients per serving: {} calories, plus unknown calories. This is a partial subtotal.", range_text(range))
        }
    });
    Ok(Estimate {
        database_version: DATABASE.version.clone(),
        known_calories: known,
        per_serving_calories: per_serving,
        unknown_ingredients,
        summary,
        per_serving_summary,
    })
}
