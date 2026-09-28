//! Deterministic estimates from the ingredient catalog. Unknown is never zero.

use std::sync::LazyLock;

use regex::Regex;

use crate::catalog::{self, normalize, Entry, Kind, Resolution};
use crate::ingredient_parser::{Measurement, ParsedIngredient};
use crate::metric_weights::parse_amount;

const RULE_VERSION: &str = "calories-v5";

static VERSION: LazyLock<String> =
    LazyLock::new(|| format!("{RULE_VERSION}-{}", catalog::version()));

/// The catalog attributes an estimate needs for one matched ingredient.
struct Food {
    kcal_per_100g: f64,
    grams_per_cup: Option<f64>,
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

/// Units that mean "a trace", never a measurable amount.
const TRACE_UNITS: [&str; 4] = ["pinch", "dash", "smidgen", "sprinkle"];

/// A line with no real amount: no numeric quantity at all ("to taste", "as
/// needed", no measurement), or only a pinch or dash.
fn is_trace_line(ingredient: &ParsedIngredient) -> bool {
    ingredient.measurements.iter().all(|measurement| {
        let trace_unit = measurement
            .unit
            .as_deref()
            .is_some_and(|unit| TRACE_UNITS.contains(&normalize(unit).as_str()));
        let has_number = measurement
            .amount
            .as_deref()
            .is_some_and(|amount| amount.chars().any(char::is_numeric));
        trace_unit || !has_number
    })
}

/// Whether a line of this entry contributes no meaningful calories: foods with
/// none (salt, water) always, and spices listed without a real amount.
fn is_negligible(entry: &Entry, ingredient: &ParsedIngredient) -> bool {
    entry.zero_calorie || (entry.trace_ok && is_trace_line(ingredient))
}

fn food(entry: &Entry) -> Result<Food, &'static str> {
    let kcal_per_100g = entry
        .fdc_id
        .and_then(catalog::food)
        .and_then(|food| food.kcal_per_100g)
        .ok_or("No supported nutrition match")?;
    Ok(Food {
        kcal_per_100g,
        grams_per_cup: entry.grams_per_cup,
    })
}

/// What one ingredient line adds to the estimate.
enum Line {
    Calories(CalorieRange),
    /// Not something eaten (a leftover header, parchment paper).
    Skipped,
}

const ZERO: CalorieRange = CalorieRange { min: 0.0, max: 0.0 };

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
    let cups = catalog::volume_to_cups(1.0, unit).ok_or("Unsupported quantity unit")?;
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

/// Oil listed "for frying" is a cooking medium: most of it is discarded, so
/// charging the whole quart would inflate the recipe by thousands of calories.
/// "2 tbsp oil, plus more for frying" still counts the measured part.
fn is_frying_medium(ingredient: &ParsedIngredient) -> bool {
    static FRYING: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\bfor\s+(?:(?:deep|shallow|pan)[- ]?)?fry(?:ing)?\b").unwrap()
    });
    FRYING.is_match(&ingredient.item)
        || ingredient
            .note
            .as_deref()
            .is_some_and(|note| FRYING.is_match(note) && !note.to_lowercase().contains("plus more"))
}

fn contribution(ingredient: &ParsedIngredient) -> Result<Line, &'static str> {
    if is_frying_medium(ingredient) {
        return Err("Frying oil: only part of it is absorbed");
    }
    let entry = match catalog::resolve_line(&ingredient.item, ingredient.note.as_deref()) {
        Resolution::Entry { entry, .. } if entry.kind == Kind::Product => return Ok(Line::Skipped),
        Resolution::Entry { entry, .. } => entry,
        Resolution::NotFood => return Ok(Line::Skipped),
        // One amount for several foods can't be split between them, so only an
        // all-negligible line ("salt and pepper") is known.
        Resolution::Compound(entries) => {
            let is_product = |entry: &&Entry| entry.kind == Kind::Product;
            // "parchment paper and aluminum foil" is equipment, not food.
            if entries.iter().all(is_product) {
                return Ok(Line::Skipped);
            }
            return if entries
                .iter()
                .all(|entry| is_product(entry) || is_negligible(entry, ingredient))
            {
                Ok(Line::Calories(ZERO))
            } else {
                Err("Several ingredients share one amount")
            };
        }
        Resolution::Ambiguous => return Err("Ambiguous ingredient"),
        Resolution::Unresolved => return Err("No supported nutrition match"),
    };
    if is_negligible(entry, ingredient) {
        return Ok(Line::Calories(ZERO));
    }
    measured_calories(ingredient, &food(entry)?).map(Line::Calories)
}

fn measured_calories(
    ingredient: &ParsedIngredient,
    food: &Food,
) -> Result<CalorieRange, &'static str> {
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
            Ok(Line::Skipped) => {}
            Ok(Line::Calories(value)) => {
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
        None if unknown_ingredients.is_empty() => "No ingredients to estimate.".to_string(),
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
        database_version: VERSION.clone(),
        known_calories: known,
        per_serving_calories: per_serving,
        unknown_ingredients,
        summary,
        per_serving_summary,
    })
}
