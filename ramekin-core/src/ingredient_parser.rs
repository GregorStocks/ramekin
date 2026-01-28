//! Ingredient parsing module.
//!
//! Parses raw ingredient strings (e.g., "2 cups flour, sifted") into structured data.

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

/// A single measurement (amount + unit pair)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Measurement {
    pub amount: Option<String>,
    pub unit: Option<String>,
}

/// Parsed ingredient structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParsedIngredient {
    pub item: String,
    pub measurements: Vec<Measurement>,
    pub note: Option<String>,
    pub raw: Option<String>,
}

/// Common cooking units (lowercase for matching).
/// Sorted by length at runtime (longest first) to avoid partial matches
/// (e.g., "tablespoons" must match before "tb").
static UNITS_SORTED: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut units = UNITS_RAW.to_vec();
    units.sort_by(|a, b| b.len().cmp(&a.len()));
    units
});

const UNITS_RAW: &[&str] = &[
    // Volume - US
    "fluid ounces",
    "fluid ounce",
    "tablespoons",
    "tablespoon",
    "teaspoons",
    "teaspoon",
    "gallons",
    "gallon",
    "quarts",
    "quart",
    "pints",
    "pint",
    "cups",
    "cup",
    "tbsp",
    "tbs",
    "tsp",
    "fl oz",
    "fl. oz",
    "gal",
    "qt",
    "pt",
    "tb",
    "ts",
    "c",
    // Volume - Metric
    "milliliters",
    "milliliter",
    "liters",
    "liter",
    "litres",
    "litre",
    "ml",
    "l",
    // Weight - US
    "ounces",
    "ounce",
    "pounds",
    "pound",
    "lbs",
    "lb",
    "oz",
    // Weight - Metric
    "kilograms",
    "kilogram",
    "milligrams",
    "milligram",
    "grams",
    "gram",
    "kg",
    "mg",
    "g",
    // Count/Size
    "packages",
    "package",
    "handfuls",
    "handful",
    "bottles",
    "bunches",
    "pinches",
    "slices",
    "sprigs",
    "stalks",
    "pieces",
    "bottle",
    "cloves",
    "dashes",
    "drops",
    "heads",
    "sticks",
    "bunch",
    "clove",
    "cubes",
    "piece",
    "pinch",
    "slice",
    "sprig",
    "stalk",
    "boxes",
    "cans",
    "jars",
    "bags",
    "cube",
    "dash",
    "drop",
    "head",
    "pkgs",
    "stick",
    "box",
    "can",
    "jar",
    "bag",
    "pcs",
    "pkg",
    "pc",
    // Size descriptors that act like units
    "extra-large",
    "medium",
    "small",
    "large",
    "xl",
];

/// Common preparation notes
const PREP_NOTES: &[&str] = &[
    "at room temperature",
    "room temperature",
    "loosely packed",
    "firmly packed",
    "lightly beaten",
    "roughly chopped",
    "coarsely chopped",
    "finely chopped",
    "thinly sliced",
    "plus more for",
    "for garnish",
    "for serving",
    "approximately",
    "julienned",
    "quartered",
    "shredded",
    "blanched",
    "crumbled",
    "softened",
    "uncooked",
    "combined",
    "or more",
    "or less",
    "optional",
    "to taste",
    "as needed",
    "chopped",
    "crushed",
    "cleaned",
    "divided",
    "drained",
    "toasted",
    "roasted",
    "trimmed",
    "whisked",
    "chilled",
    "minced",
    "sliced",
    "grated",
    "melted",
    "cooked",
    "ground",
    "beaten",
    "thawed",
    "frozen",
    "peeled",
    "washed",
    "rinsed",
    "packed",
    "sifted",
    "halved",
    "diced",
    "cubed",
    "cored",
    "mixed",
    "fresh",
    "dried",
    "whole",
    "cold",
    "raw",
];

/// Parse a single ingredient line into structured data.
///
/// This does best-effort parsing - if we can't parse something meaningful,
/// we return the raw text as the item with empty measurements.
pub fn parse_ingredient(raw: &str) -> ParsedIngredient {
    let raw = raw.trim();
    if raw.is_empty() {
        return ParsedIngredient {
            item: String::new(),
            measurements: vec![],
            note: None,
            raw: Some(raw.to_string()),
        };
    }

    let mut remaining = raw.to_string();
    let mut measurements = Vec::new();
    let mut note = None;

    // Step 1: Extract any parenthetical alternative measurements
    // e.g., "1 stick (113g) butter" -> extract "(113g)"
    let mut alt_measurements = Vec::new();
    while let Some(start) = remaining.find('(') {
        if let Some(end) = remaining[start..].find(')') {
            let paren_content = &remaining[start + 1..start + end];
            // Try to parse as a measurement
            if let Some(m) = try_parse_measurement(paren_content) {
                alt_measurements.push(m);
                // Remove the parenthetical from remaining, preserving space
                let before = remaining[..start].trim_end();
                let after = remaining[start + end + 1..].trim_start();
                remaining = if before.is_empty() {
                    after.to_string()
                } else if after.is_empty() {
                    before.to_string()
                } else {
                    format!("{} {}", before, after)
                };
            } else {
                // Not a measurement, leave it and stop looking for more
                break;
            }
        } else {
            break;
        }
    }

    // Step 2: Extract primary amount from the beginning
    let (primary_amount, after_amount) = extract_amount(&remaining);
    remaining = after_amount;

    // Step 3: Extract primary unit
    let (primary_unit, after_unit) = extract_unit(&remaining);
    remaining = after_unit;

    // Step 4: Extract note from the end (after comma)
    if let Some(comma_idx) = remaining.rfind(',') {
        let potential_note = remaining[comma_idx + 1..].trim();
        // Check if it looks like a prep note
        if is_prep_note(potential_note) {
            note = Some(potential_note.to_string());
            remaining = remaining[..comma_idx].trim().to_string();
        }
    }

    // Step 5: Build measurements list
    if primary_amount.is_some() || primary_unit.is_some() {
        measurements.push(Measurement {
            amount: primary_amount,
            unit: primary_unit,
        });
    }
    measurements.extend(alt_measurements);

    // Step 6: The remaining text is the ingredient item
    let item = remaining.trim().to_string();

    // If we didn't extract anything useful, just use raw as item
    if item.is_empty() && measurements.is_empty() {
        return ParsedIngredient {
            item: raw.to_string(),
            measurements: vec![],
            note: None,
            raw: Some(raw.to_string()),
        };
    }

    ParsedIngredient {
        item: if item.is_empty() {
            raw.to_string()
        } else {
            item
        },
        measurements,
        note,
        raw: Some(raw.to_string()),
    }
}

/// Try to parse a string as a measurement (amount + optional unit).
fn try_parse_measurement(s: &str) -> Option<Measurement> {
    let s = s.trim();
    let (amount, after_amount) = extract_amount(s);
    let (unit, _) = extract_unit(&after_amount);

    if amount.is_some() || unit.is_some() {
        Some(Measurement { amount, unit })
    } else {
        None
    }
}

/// Extract an amount from the beginning of a string.
/// Returns (amount, remaining_string).
fn extract_amount(s: &str) -> (Option<String>, String) {
    let s = s.trim();
    if s.is_empty() {
        return (None, s.to_string());
    }

    // Check for mixed number: "1 1/2" pattern
    // We need to look for: number, space, fraction
    let words: Vec<&str> = s.split_whitespace().collect();
    if words.len() >= 2 {
        let first = words[0];
        let second = words[1];

        // Check if first is a whole number and second is a fraction
        if first.chars().all(|c| c.is_ascii_digit()) && is_fraction(second) {
            let amount = format!("{} {}", first, second);
            // Find where the second word ends in the original string
            if let Some(pos) = s.find(second) {
                let end_pos = pos + second.len();
                return (Some(amount), s[end_pos..].trim().to_string());
            }
        }
    }

    // Check for fraction at start: "1/2"
    if let Some(first_word) = words.first() {
        if is_fraction(first_word) {
            let word_len = first_word.len();
            return (
                Some((*first_word).to_string()),
                s[word_len..].trim().to_string(),
            );
        }
    }

    // Check for decimal or integer at start
    let mut chars = s.chars().peekable();
    let mut amount_str = String::new();

    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() || c == '.' {
            amount_str.push(c);
            chars.next();
        } else {
            break;
        }
    }

    if !amount_str.is_empty() && amount_str != "." {
        let remaining: String = chars.collect();
        return (Some(amount_str), remaining.trim().to_string());
    }

    (None, s.to_string())
}

/// Check if a string is a fraction like "1/2" or "3/4"
fn is_fraction(s: &str) -> bool {
    if let Some(slash_pos) = s.find('/') {
        let before = &s[..slash_pos];
        let after = &s[slash_pos + 1..];
        !before.is_empty()
            && !after.is_empty()
            && before.chars().all(|c| c.is_ascii_digit())
            && after.chars().all(|c| c.is_ascii_digit())
    } else {
        false
    }
}

/// Extract a unit from the beginning of a string.
/// Returns (unit, remaining_string).
fn extract_unit(s: &str) -> (Option<String>, String) {
    let s = s.trim();
    let s_lower = s.to_lowercase();

    for &unit in UNITS_SORTED.iter() {
        if s_lower.starts_with(unit) {
            // Make sure it's a word boundary
            let after = &s[unit.len()..];
            if after.is_empty()
                || after.starts_with(|c: char| c.is_whitespace() || c == '.' || c == ',')
            {
                // Skip any trailing period or whitespace
                let remaining = after.trim_start_matches('.').trim();
                return (Some(unit.to_string()), remaining.to_string());
            }
        }
    }

    (None, s.to_string())
}

/// Check if a string looks like a preparation note.
fn is_prep_note(s: &str) -> bool {
    let s_lower = s.to_lowercase();
    PREP_NOTES.iter().any(|note| s_lower.contains(note))
}

/// Parse multiple ingredient lines (separated by newlines).
pub fn parse_ingredients(blob: &str) -> Vec<ParsedIngredient> {
    blob.lines()
        .filter(|line| !line.trim().is_empty())
        .map(parse_ingredient)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_ingredient() {
        let result = parse_ingredient("2 cups flour");
        assert_eq!(result.item, "flour");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cups".to_string()));
    }

    #[test]
    fn test_ingredient_with_note() {
        let result = parse_ingredient("1 cup butter, softened");
        assert_eq!(result.item, "butter");
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
        assert_eq!(result.note, Some("softened".to_string()));
    }

    #[test]
    fn test_ingredient_with_fraction() {
        let result = parse_ingredient("1/2 cup sugar");
        assert_eq!(result.item, "sugar");
        assert_eq!(result.measurements[0].amount, Some("1/2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_ingredient_with_mixed_number() {
        let result = parse_ingredient("1 1/2 cups water");
        assert_eq!(result.item, "water");
        assert_eq!(result.measurements[0].amount, Some("1 1/2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cups".to_string()));
    }

    #[test]
    fn test_ingredient_with_alternative_measurement() {
        let result = parse_ingredient("1 stick (113g) butter");
        assert_eq!(result.item, "butter");
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("stick".to_string()));
        assert_eq!(result.measurements[1].amount, Some("113".to_string()));
        assert_eq!(result.measurements[1].unit, Some("g".to_string()));
    }

    #[test]
    fn test_ingredient_no_amount() {
        let result = parse_ingredient("Salt to taste");
        assert_eq!(result.item, "Salt to taste");
        assert!(result.measurements.is_empty());
    }

    #[test]
    fn test_ingredient_no_unit() {
        let result = parse_ingredient("3 eggs");
        assert_eq!(result.item, "eggs");
        assert_eq!(result.measurements[0].amount, Some("3".to_string()));
        assert_eq!(result.measurements[0].unit, None);
    }

    #[test]
    fn test_ingredient_decimal_amount() {
        let result = parse_ingredient("2.5 oz cream cheese");
        assert_eq!(result.item, "cream cheese");
        assert_eq!(result.measurements[0].amount, Some("2.5".to_string()));
        assert_eq!(result.measurements[0].unit, Some("oz".to_string()));
    }

    #[test]
    fn test_empty_ingredient() {
        let result = parse_ingredient("");
        assert_eq!(result.item, "");
        assert!(result.measurements.is_empty());
    }

    #[test]
    fn test_parse_multiple_ingredients() {
        let blob = "2 cups flour\n1 cup sugar\n3 eggs";
        let results = parse_ingredients(blob);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].item, "flour");
        assert_eq!(results[1].item, "sugar");
        assert_eq!(results[2].item, "eggs");
    }

    #[test]
    fn test_tablespoon_abbreviations() {
        let result = parse_ingredient("1 tbsp olive oil");
        assert_eq!(result.item, "olive oil");
        assert_eq!(result.measurements[0].unit, Some("tbsp".to_string()));

        let result = parse_ingredient("2 tablespoons butter");
        assert_eq!(result.item, "butter");
        assert_eq!(result.measurements[0].unit, Some("tablespoons".to_string()));
    }

    #[test]
    fn test_preserves_raw() {
        let result = parse_ingredient("2 cups flour, sifted");
        assert_eq!(result.raw, Some("2 cups flour, sifted".to_string()));
    }
}
