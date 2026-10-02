//! Deterministic estimates from the ingredient catalog. Unknown is never zero.

use std::sync::LazyLock;

use regex::Regex;

use crate::catalog::{self, normalize, Entry, Kind, Resolution};
use crate::ingredient_parser::{Measurement, ParsedIngredient};
use crate::metric_weights::parse_amount;

const RULE_VERSION: &str = "calories-v12";

static VERSION: LazyLock<String> =
    LazyLock::new(|| format!("{RULE_VERSION}-{}", catalog::version()));

/// The catalog attributes an estimate needs for one matched ingredient.
struct Food<'a> {
    kcal_per_100g: f64,
    /// Supplies the density and per-piece weights.
    entry: &'a Entry,
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

/// How far to trust an estimate, from how many real ingredients it couldn't count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Every ingredient is counted or negligible.
    Complete,
    /// A few ingredients are missing, so the total is a lower bound.
    Partial,
    /// Too many ingredients are missing for a useful number.
    Insufficient,
    /// Nothing to estimate (no ingredients, or only headers and products).
    Empty,
}

/// One ingredient line's part of the estimate, for the breakdown.
#[derive(Debug)]
pub struct LineEstimate {
    pub index: usize,
    pub item: String,
    /// Scaled calories, when counted (zero for negligible lines).
    pub calories: Option<CalorieRange>,
    /// "~120 kcal", "Negligible", "Not a food", or why it couldn't be counted.
    pub text: String,
}

#[derive(Debug)]
pub struct Estimate {
    pub database_version: String,
    pub status: Status,
    /// The one line to show: "~520 kcal per serving", "At least ~3,100 kcal
    /// for the whole recipe", "Not enough ingredient data to estimate calories".
    pub headline: String,
    /// The whole-recipe figure under a per-serving headline, or how many
    /// ingredients an insufficient estimate is missing.
    pub secondary: Option<String>,
    /// Ingredients the figures leave out: those given no amount, and for a
    /// partial estimate the ones that couldn't be counted.
    pub not_counted: Vec<String>,
    pub known_calories: Option<CalorieRange>,
    pub per_serving_calories: Option<CalorieRange>,
    /// Internal reasons, for the audit; clients get `lines`.
    pub unknown_ingredients: Vec<UnknownIngredient>,
    /// Lines given no amount at all ("olive oil", "lime wedges, to serve"):
    /// not counted, but not unknown either, so they never make an estimate
    /// partial or insufficient. Internal reasons, as above.
    pub no_amount: Vec<UnknownIngredient>,
    pub lines: Vec<LineEstimate>,
}

/// The most uncounted ingredients an estimate can have and still show a
/// (lower-bound) number. See doc/calorie-estimates.md for how it was chosen.
pub const MAX_UNKNOWN_LINES: usize = 3;

/// Units that mean "a trace", never a measurable amount.
const TRACE_UNITS: [&str; 5] = ["pinch", "dash", "smidgen", "sprinkle", "sprig"];

/// Units that count small whole pieces of a trace food ("2 bay leaves", "1
/// cinnamon stick"); no unit is a bare count.
const TRACE_COUNT_UNITS: [&str; 16] = [
    "", "leaf", "leaves", "stick", "sticks", "pod", "pods", "stem", "stems", "piece", "pieces",
    "whole", "clove", "cloves", "star", "stars",
];

/// A line given no amount at all: no measurement, or none with an amount
/// ("olive oil", "lime wedges, to serve"). An unclear amount ("a handful")
/// is not this; it stays unknown.
fn has_no_amount(ingredient: &ParsedIngredient) -> bool {
    ingredient.measurements.iter().all(|measurement| {
        measurement
            .amount
            .as_deref()
            .is_none_or(|amount| amount.trim().is_empty())
    })
}

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
/// Oils and solid frying fats; only these are discarded as a frying medium
/// ("2 cups flour, for frying" is dredging flour that stays in the dish).
fn is_cooking_fat(entry: &Entry) -> bool {
    entry.fdc_id.and_then(catalog::food).is_some_and(|food| {
        let description = food.description.as_str();
        description.starts_with("oil")
            || description.starts_with("lard")
            || description.starts_with("shortening")
    })
}

/// The most pieces of a trace food, after scaling, that still count as a trace
/// ("6-8 bay leaves"). Beyond this, the calories are unknown rather than zero.
const MAX_TRACE_COUNT: f64 = 10.0;

/// A few whole leaves, sticks, or pods of a food USDA has no piece weight for
/// ("2 bay leaves"); a handful of a trace food is a few calories.
fn is_unweighed_count(entry: &Entry, ingredient: &ParsedIngredient, scale: f64) -> bool {
    ingredient.measurements.iter().all(|measurement| {
        let few = measurement
            .amount
            .as_deref()
            .and_then(quantity)
            .is_some_and(|count| count.max * scale <= MAX_TRACE_COUNT);
        let unit = measurement
            .unit
            .as_deref()
            .map(normalize)
            .unwrap_or_default();
        few && TRACE_COUNT_UNITS.contains(&unit.as_str())
            && catalog::grams_per_piece(entry, Some(unit.as_str())).is_none()
    })
}

/// The most volume of a trace food with no density, after scaling, that still
/// counts as a trace: a tablespoon of most spices is 15-25 kcal, and of dense
/// seeds such as whole mustard seed up to about 50.
const MAX_TRACE_CUPS: f64 = catalog::CUPS_PER_TBSP;

/// Up to a tablespoon of a trace food whose volume can't be weighed ("1 tsp
/// freshly ground black pepper"; grind size makes pepper's density unusable).
/// A food with a density is computed normally.
fn is_unweighed_spoonful(entry: &Entry, ingredient: &ParsedIngredient, scale: f64) -> bool {
    let cups = |measurement: &Measurement| {
        let cups_per_unit = |unit: &str| {
            catalog::volume_to_cups(1.0, &canonical_unit(&normalize(unit)))
                .map(exact)
                .ok_or("Not a volume")
        };
        measurement.amount.as_deref().and_then(|amount| {
            measurement_total(amount, measurement.unit.as_deref(), cups_per_unit).ok()
        })
    };
    // Measurements are alternatives and the primary one leads, as in
    // `measured_calories`. A weight alternative ("1 tsp / 2 g") is counted
    // instead whenever the food has calories to weigh it with.
    let Some(primary) = ingredient.measurements.first() else {
        return false;
    };
    entry.grams_per_cup.is_none()
        && cups(primary).is_some_and(|cups| cups.max * scale <= MAX_TRACE_CUPS)
        && (entry.kcal_per_100g.is_none()
            || ingredient.measurements.iter().all(|m| cups(m).is_some()))
}

fn is_negligible(entry: &Entry, ingredient: &ParsedIngredient, scale: f64) -> bool {
    entry.zero_calorie
        || (entry.trace_ok
            && (is_trace_line(ingredient)
                || is_unweighed_count(entry, ingredient, scale)
                || is_unweighed_spoonful(entry, ingredient, scale)))
}

fn food(entry: &Entry) -> Result<Food<'_>, &'static str> {
    let kcal_per_100g = entry.kcal_per_100g.ok_or("No supported nutrition match")?;
    Ok(Food {
        kcal_per_100g,
        entry,
    })
}

/// What one ingredient line adds to the estimate.
enum Line {
    Calories(CalorieRange),
    /// Too little to matter (salt, a pinch of spice, a few bay leaves).
    Negligible,
    /// Not something eaten (a leftover header, parchment paper).
    Skipped,
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

/// Containers whose weight a recipe may state ("15-ounce can").
const PACKAGE_WORDS: &str = "can|package|block|bag|jar|box|carton|container|tin|bottle|packet|tub";

/// Mass units a package weight may use.
const PACKAGE_MASS: &str = "ounces?|oz|pounds?|lbs?|grams?|g";

/// "2 (15-ounce) cans" parses as unit "can" with the weight leading the note
/// ("15-ounce, drained"); join them back into one weighed unit.
fn noted_package_unit(unit: &str, note: Option<&str>) -> Option<String> {
    static BARE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(&format!(r"^(?:{PACKAGE_WORDS})s?$")).unwrap());
    static WEIGHT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(&format!(r"^(.+?)[- ]?({PACKAGE_MASS})\.?(?:$|[,;]| or )")).unwrap()
    });
    let unit = normalize(unit);
    if !BARE.is_match(&unit) {
        return None;
    }
    let note = normalize(note?);
    let weight = WEIGHT.captures(&note)?;
    Some(format!("{} {} {unit}", &weight[1], &weight[2]))
}

/// A unit that carries its own weight: "15-ounce can", "(28-oz.) can", "14
/// 1/2-ounce can", "12- to 18-ounce package", "425-gram package".
fn package_grams(unit: &str) -> Option<CalorieRange> {
    static PACKAGE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(&format!(
            r"^\(?(.+?)[- ]?({PACKAGE_MASS})\.?\)?[- ]?(?:{PACKAGE_WORDS})s?$"
        ))
        .unwrap()
    });
    let parts = PACKAGE.captures(unit)?;
    // "12- to 18-ounce" shares the unit between both ends of the range.
    let weight = quantity(&parts[1].replace("- to ", " to "))?;
    let grams_per = match parts[2].trim_end_matches('s') {
        "ounce" | "oz" => 28.349523125,
        "pound" | "lb" => 453.59237,
        _ => 1.0,
    };
    Some(CalorieRange {
        min: weight.min * grams_per,
        max: weight.max * grams_per,
    })
}

fn exact(grams: f64) -> CalorieRange {
    CalorieRange {
        min: grams,
        max: grams,
    }
}

/// A normalized unit with any fill word dropped (a heaped or scant spoon is
/// close enough to a level one) and volume names in the catalog's spelling
/// ("tablespoons" -> "tbsp").
fn canonical_unit(normalized: &str) -> String {
    static FILL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?:slightly )?(?:heaped|heaping|scant|generous|level|rounded) ").unwrap()
    });
    let unit = FILL.replace(normalized, "");
    match unit.as_ref() {
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
    }
    .to_string()
}

fn grams_per_unit(unit: &str, food: &Food) -> Result<CalorieRange, &'static str> {
    let normalized = normalize(unit);
    if normalized.is_empty() {
        return catalog::grams_per_piece(food.entry, None)
            .map(exact)
            .ok_or("Unsupported quantity unit");
    }
    if let Some(grams) = package_grams(&normalized) {
        return Ok(grams);
    }
    let unit = canonical_unit(&normalized);
    match unit.as_str() {
        "gram" | "grams" | "g" => return Ok(exact(1.0)),
        "kilogram" | "kilograms" | "kg" => return Ok(exact(1000.0)),
        "milligram" | "milligrams" | "mg" => return Ok(exact(0.001)),
        "ounce" | "ounces" | "oz" => return Ok(exact(28.349523125)),
        "pound" | "pounds" | "lb" | "lbs" => return Ok(exact(453.59237)),
        _ => {}
    }
    let unit = unit.as_str();
    let Some(cups) = catalog::volume_to_cups(1.0, unit) else {
        return catalog::grams_per_piece(food.entry, Some(unit))
            .map(exact)
            .ok_or("Unsupported quantity unit");
    };
    Ok(exact(
        cups * food
            .entry
            .grams_per_cup
            .ok_or("Missing density for this food")?,
    ))
}

fn measurement_grams(
    amount: &str,
    unit: Option<&str>,
    food: &Food,
) -> Result<CalorieRange, &'static str> {
    measurement_total(amount, unit, |unit| grams_per_unit(unit, food))
}

/// A measurement's total in whatever `per_unit` converts one unit to,
/// including compound amounts ("1 cup plus 2 tbsp").
fn measurement_total(
    amount: &str,
    unit: Option<&str>,
    per_unit: impl Fn(&str) -> Result<CalorieRange, &'static str>,
) -> Result<CalorieRange, &'static str> {
    static PLUS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+(?:plus|\+)\s+").unwrap());
    static EMBEDDED: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(.+?)\s+([a-z][a-z ]*)$").unwrap());
    let amount = normalize(amount);
    let mut total = CalorieRange { min: 0.0, max: 0.0 };
    for segment in PLUS.split(&amount) {
        let (range, factor) = if let Some(range) = quantity(segment) {
            (range, per_unit(unit.unwrap_or(""))?)
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
            (range, per_unit(&parts[2])?)
        };
        total.min += range.min * factor.min;
        total.max += range.max * factor.max;
    }
    Ok(total)
}

/// Above this, oil listed "for frying" is a frying medium (about 1/4 cup of oil).
const FRYING_KEPT_KCAL: f64 = 500.0;

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

fn contribution(
    ingredient: &ParsedIngredient,
    scale: f64,
    learned: &catalog::Learned,
) -> Result<Line, &'static str> {
    let entry =
        match catalog::resolve_line_with(&ingredient.item, ingredient.note.as_deref(), learned) {
            Resolution::Entry { entry, .. } if entry.kind == Kind::Product => {
                return Ok(Line::Skipped)
            }
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
                    .all(|entry| is_product(entry) || is_negligible(entry, ingredient, scale))
                {
                    Ok(Line::Negligible)
                } else {
                    Err("Several ingredients share one amount")
                };
            }
            Resolution::Ambiguous => return Err("Ambiguous ingredient"),
            Resolution::Unresolved => return Err("No supported nutrition match"),
        };
    if is_negligible(entry, ingredient, scale) {
        return Ok(Line::Negligible);
    }
    let calories = measured_calories(ingredient, &food(entry)?);
    if is_frying_medium(ingredient) && is_cooking_fat(entry) {
        // A deep-frying amount is mostly discarded; a spoonful for browning
        // stays in the dish.
        return match calories {
            Ok(range) if range.max <= FRYING_KEPT_KCAL => Ok(Line::Calories(range)),
            _ => Err("Frying oil: only part of it is absorbed"),
        };
    }
    calories.map(Line::Calories)
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
        let noted = unit
            .as_deref()
            .and_then(|unit| noted_package_unit(unit, ingredient.note.as_deref()));
        let grams = match measurement_grams(amount, noted.as_deref().or(unit.as_deref()), food) {
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

/// A calorie count as people read it: whole calories under 100, tens above,
/// with thousands separators ("~5", "~120", "~3,100").
/// Which way a displayed number may move: to the nearest value for a single
/// estimate, or outward for a range's ends and lower bounds, so the display
/// never claims more (or less) than was computed.
#[derive(Clone, Copy)]
enum Rounding {
    Nearest,
    Down,
    Up,
}

fn kcal_number(kcal: f64, rounding: Rounding) -> String {
    let step = if kcal < 100.0 { 1.0 } else { 10.0 };
    let rounded = match rounding {
        Rounding::Nearest => (kcal / step).round() * step,
        Rounding::Down => (kcal / step).floor() * step,
        Rounding::Up => (kcal / step).ceil() * step,
    };
    // A single tiny estimate is "<1"; a range's lower end may honestly be 0.
    if matches!(rounding, Rounding::Nearest) && kcal > 0.0 && rounded == 0.0 {
        return "<1".to_string();
    }
    let digits = format!("{rounded:.0}");
    let mut grouped = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

/// "~120 kcal", or "~480–560 kcal" for a range (rounded outward).
fn kcal_text(range: CalorieRange) -> String {
    if range.max > 0.0 && range.max < 1.0 {
        return "<1 kcal".to_string();
    }
    let (min, max) = if range.min == range.max {
        let value = kcal_number(range.min, Rounding::Nearest);
        (value.clone(), value)
    } else {
        (
            kcal_number(range.min, Rounding::Down),
            kcal_number(range.max, Rounding::Up),
        )
    };
    if min == max && min == "<1" {
        "<1 kcal".to_string()
    } else if min == max {
        format!("~{min} kcal")
    } else {
        format!("~{min}–{max} kcal")
    }
}

/// Why a line couldn't be counted, in words for the breakdown.
fn unknown_label(reason: &str) -> &'static str {
    match reason {
        "No supported nutrition match" => "Not recognized",
        "Ambiguous ingredient" => "Could be several foods",
        "Several ingredients share one amount" => "Several foods share one amount",
        "Missing quantity" | "Unsupported or missing quantity" | "Unsupported quantity unit" => {
            "Amount unclear"
        }
        "Missing density for this food" => "Can't convert this measurement to weight",
        "Frying oil: only part of it is absorbed" => "Frying oil: only part is absorbed",
        other => panic!("no label for unknown reason {other:?}"),
    }
}

/// How many servings a recipe makes, as a range ("4 to 6 servings" is 4–6).
/// Accepts "serves 4", "servings: 4", "servings 4", "yield: 4", "makes 4
/// servings", and a bare count, with an optional "servings", "people",
/// "person(s)", or "portion(s)" suffix. "Makes 12 cookies" and a bare "makes
/// 24" name no servings, so they're None.
fn serving_count(servings: &str) -> Option<CalorieRange> {
    static PREFIX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?:serves|servings?|yields?|makes)(?:\s*:\s*|\s+)").unwrap()
    });
    static SUFFIX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\s+(?:servings?|people|persons?|portions?)$").unwrap());
    let text = normalize(servings);
    // "Makes 24" is usually cookies; only "makes 4 servings" names servings.
    if text.starts_with("makes") && !SUFFIX.is_match(&text) {
        return None;
    }
    let count = PREFIX.replace(&text, "");
    let count = SUFFIX.replace(&count, "");
    quantity(&count).filter(|r| r.min > 0.0 && r.min <= r.max)
}

/// `estimate_with` using only the committed catalog.
pub fn estimate(
    ingredients: &[ParsedIngredient],
    servings: Option<&str>,
    scale: f64,
) -> Result<Estimate, &'static str> {
    estimate_with(ingredients, servings, scale, &catalog::Learned::new())
}

/// All arithmetic and presentation are computed here; clients render the result.
/// `learned` holds stored answers for names the committed catalog doesn't know.
pub fn estimate_with(
    ingredients: &[ParsedIngredient],
    servings: Option<&str>,
    scale: f64,
    learned: &catalog::Learned,
) -> Result<Estimate, &'static str> {
    if !scale.is_finite() || scale <= 0.0 || scale > 1e6 {
        return Err("Scale must be finite, greater than zero, and at most 1000000");
    }
    let mut known = None;
    let mut unknown_ingredients = Vec::new();
    let mut no_amount = Vec::new();
    let mut lines = Vec::new();
    for (index, ingredient) in ingredients.iter().enumerate() {
        let (calories, text) = match contribution(ingredient, scale, learned) {
            Ok(Line::Skipped) => (None, "Not a food".to_string()),
            Ok(Line::Negligible) => {
                known.get_or_insert(CalorieRange { min: 0.0, max: 0.0 });
                (
                    Some(CalorieRange { min: 0.0, max: 0.0 }),
                    "Negligible".to_string(),
                )
            }
            Ok(Line::Calories(value)) => {
                let total = known.get_or_insert(CalorieRange { min: 0.0, max: 0.0 });
                total.min += value.min;
                total.max += value.max;
                if !total.min.is_finite() || !total.max.is_finite() || total.max * scale > 1e15 {
                    return Err("Calorie estimate exceeds supported numeric bounds");
                }
                let scaled = CalorieRange {
                    min: value.min * scale,
                    max: value.max * scale,
                };
                (Some(scaled), kcal_text(scaled))
            }
            Err(reason) => {
                let uncounted = UnknownIngredient {
                    index,
                    item: ingredient.item.clone(),
                    reason: reason.to_string(),
                };
                // With no amount there is nothing to count, whatever the food:
                // left out and listed, not unknown.
                if has_no_amount(ingredient) {
                    no_amount.push(uncounted);
                    (None, "No amount given".to_string())
                } else {
                    unknown_ingredients.push(uncounted);
                    (None, unknown_label(reason).to_string())
                }
            }
        };
        lines.push(LineEstimate {
            index,
            item: ingredient.item.clone(),
            calories,
            text,
        });
    }
    // Divide the original subtotal directly: scaling ingredients and servings
    // cancels out, including at very small scales where a scaled subtotal underflows.
    let count = servings.and_then(serving_count);
    // "4 to 6 servings": the fewest calories per serving come from the most
    // servings, and the most from the fewest.
    let per_serving = known.zip(count).map(|(total, count)| CalorieRange {
        min: total.min / count.max,
        max: total.max / count.min,
    });
    if per_serving.is_some_and(|range| !range.max.is_finite() || range.max > 1e15) {
        return Err("Per-serving estimate exceeds supported numeric bounds");
    }
    let known = known.map(|total| CalorieRange {
        min: total.min * scale,
        max: total.max * scale,
    });
    let status = match (known, unknown_ingredients.len()) {
        (None, 0) if no_amount.is_empty() => Status::Empty,
        (None, _) => Status::Insufficient,
        // Only lines with no amount left out: a whole-recipe figure, unless
        // what was counted is under 1 kcal (only salt and "olive oil").
        (Some(total), 0) if no_amount.is_empty() || total.min >= 1.0 => Status::Complete,
        // A lower bound under 1 kcal (only salt, "0-100 g", a pinch of sugar)
        // says nothing.
        (Some(total), unknown)
            if unknown <= MAX_UNKNOWN_LINES
                && total.min >= 1.0
                && per_serving.is_none_or(|serving| serving.min >= 1.0) =>
        {
            Status::Partial
        }
        (Some(_), _) => Status::Insufficient,
    };
    // A partial total is a lower bound, so it shows only its minimum.
    let figure = |range: CalorieRange, what: &str| match status {
        // Rounded down, so it never claims more than was counted.
        Status::Partial => format!(
            "At least ~{} kcal {what}",
            kcal_number(range.min, Rounding::Down)
        ),
        _ => format!("{} {what}", kcal_text(range)),
    };
    let (headline, secondary) = match (status, known, per_serving) {
        (Status::Empty, ..) => ("No ingredients to estimate".to_string(), None),
        (Status::Insufficient, ..) => (
            "Not enough ingredient data to estimate calories".to_string(),
            Some(match unknown_ingredients.len() + no_amount.len() {
                1 => "1 ingredient couldn't be counted.".to_string(),
                n => format!("{n} ingredients couldn't be counted."),
            }),
        ),
        (_, Some(total), Some(serving)) => (
            figure(serving, "per serving"),
            Some(figure(total, "for the whole recipe")),
        ),
        (_, Some(total), None) => (figure(total, "for the whole recipe"), None),
        (_, None, _) => unreachable!("complete and partial estimates have a total"),
    };
    let mut left_out: Vec<&UnknownIngredient> = match status {
        Status::Partial => unknown_ingredients.iter().chain(&no_amount).collect(),
        Status::Complete => no_amount.iter().collect(),
        Status::Insufficient | Status::Empty => Vec::new(),
    };
    left_out.sort_by_key(|line| line.index);
    let not_counted = left_out.into_iter().map(|line| line.item.clone()).collect();
    Ok(Estimate {
        database_version: VERSION.clone(),
        status,
        headline,
        secondary,
        not_counted,
        known_calories: known,
        per_serving_calories: per_serving,
        unknown_ingredients,
        no_amount,
        lines,
    })
}
