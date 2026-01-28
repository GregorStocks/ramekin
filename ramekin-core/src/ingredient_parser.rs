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
    "scrubbed",
];

/// Convert unicode fractions to ASCII equivalents.
fn normalize_unicode_fractions(s: &str) -> String {
    s.replace('½', "1/2")
        .replace('⅓', "1/3")
        .replace('⅔', "2/3")
        .replace('¼', "1/4")
        .replace('¾', "3/4")
        .replace('⅕', "1/5")
        .replace('⅖', "2/5")
        .replace('⅗', "3/5")
        .replace('⅘', "4/5")
        .replace('⅙', "1/6")
        .replace('⅚', "5/6")
        .replace('⅛', "1/8")
        .replace('⅜', "3/8")
        .replace('⅝', "5/8")
        .replace('⅞', "7/8")
}

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

    // Normalize unicode fractions before processing
    let mut remaining = normalize_unicode_fractions(raw);
    let mut measurements = Vec::new();
    let mut note = None;

    // Step 1: Extract any parenthetical content (measurements or prep notes)
    // e.g., "1 stick (113g) butter" -> extract "(113g)" as alt measurement
    // e.g., "1/2 cup butter (softened)" -> extract "(softened)" as note
    let mut alt_measurements = Vec::new();
    while let Some(start) = remaining.find('(') {
        if let Some(end) = remaining[start..].find(')') {
            let paren_content = &remaining[start + 1..start + end];

            // First check if this is a prep note (like "softened", "chopped", etc.)
            if is_prep_note(paren_content) && note.is_none() {
                note = Some(paren_content.trim().to_string());
                // Remove the parenthetical from remaining
                let before = remaining[..start].trim_end();
                let after = remaining[start + end + 1..].trim_start();
                remaining = if before.is_empty() {
                    after.to_string()
                } else if after.is_empty() {
                    before.to_string()
                } else {
                    format!("{} {}", before, after)
                };
                continue;
            }

            // Try to parse the parenthetical content as one or more measurements
            // Split by semicolons or commas to handle "8 ounces; 227 g each"
            let parsed_measurements = parse_parenthetical_measurements(paren_content);

            if !parsed_measurements.is_empty() {
                alt_measurements.extend(parsed_measurements);
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
                // Not a measurement or prep note, leave it and stop looking for more
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
/// Preserves "each" qualifier as part of the unit (e.g., "8 ounces each" -> unit: "ounces each")
fn try_parse_measurement(s: &str) -> Option<Measurement> {
    let s = s.trim();
    let (amount, after_amount) = extract_amount(s);
    let (unit, remaining) = extract_unit(&after_amount);

    // Check if remaining is "each" - if so, append it to the unit
    // This preserves important semantic info like "8 ounces each" vs "8 ounces total"
    let unit = match (unit, remaining.trim().to_lowercase().as_str()) {
        (Some(u), "each") => Some(format!("{} each", u)),
        (u, _) => u,
    };

    if amount.is_some() || unit.is_some() {
        Some(Measurement { amount, unit })
    } else {
        None
    }
}

/// Parse parenthetical content that may contain multiple measurements.
/// Handles formats like "8 ounces; 227 g each" or "113g, 1/2 cup" or "8 ounces or 225 grams"
fn parse_parenthetical_measurements(content: &str) -> Vec<Measurement> {
    let mut results = Vec::new();

    // Check if the content ends with "each" - this applies to ALL measurements
    // e.g., "8 ounces; 227 g each" means both are per-item
    let content_lower = content.to_lowercase();
    let has_trailing_each = content_lower.trim().ends_with(" each")
        || content_lower.trim().ends_with(";each")
        || content_lower.trim().ends_with(",each");

    // First, normalize " or " to ";" for splitting (but not "or" within words)
    let normalized = content
        .replace(" or ", ";")
        .replace(" Or ", ";")
        .replace(" OR ", ";");

    // Split by semicolons or commas (common separators in recipe measurements)
    for part in normalized.split(|c| c == ';' || c == ',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        // Strip common qualifiers (but not "each" - handled separately)
        let cleaned = strip_measurement_qualifiers(part);

        if let Some(mut m) = try_parse_measurement(&cleaned) {
            // Only add if we got a meaningful measurement (has amount or recognized unit)
            if m.amount.is_some() || m.unit.is_some() {
                // If the entire parenthetical had trailing "each", apply it to all measurements
                // unless this measurement already has "each"
                if has_trailing_each {
                    if let Some(ref unit) = m.unit {
                        if !unit.ends_with(" each") {
                            m.unit = Some(format!("{} each", unit));
                        }
                    }
                }
                results.push(m);
            }
        }
    }

    results
}

/// Strip common qualifiers from measurement strings, but preserve "each" as a unit suffix.
/// e.g., "about 1 cup" -> "1 cup", "227 g each" -> "227 g each" (preserved)
fn strip_measurement_qualifiers(s: &str) -> String {
    // Qualifiers to remove completely (they don't change the meaning)
    let remove_qualifiers = [
        " total",
        " about",
        " approximately",
        " approx",
        " roughly",
        " or so",
    ];

    let mut result = s.to_string();
    for q in remove_qualifiers {
        if let Some(idx) = result.to_lowercase().find(q) {
            result = result[..idx].to_string();
        }
    }

    // Also handle qualifiers at the start
    let start_qualifiers = ["about ", "approximately ", "approx ", "roughly ", "~"];
    let lower = result.to_lowercase();
    for q in start_qualifiers {
        if lower.starts_with(q) {
            result = result[q.len()..].to_string();
            break;
        }
    }

    result.trim().to_string()
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

    #[test]
    fn test_medium_onions() {
        let result = parse_ingredient("2 medium onions");
        println!("{:#?}", result);
        assert_eq!(result.item, "onions");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("medium".to_string()));
    }

    #[test]
    fn test_medium_onions_with_weight() {
        // Test ingredient with size descriptor and weight in parens
        let result = parse_ingredient("2 medium (8 oz) onions");
        println!("{:#?}", result);
        assert_eq!(result.item, "onions");
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("medium".to_string()));
        assert_eq!(result.measurements[1].amount, Some("8".to_string()));
        assert_eq!(result.measurements[1].unit, Some("oz".to_string()));
    }

    #[test]
    fn test_seriouseats_onion_format() {
        // Real input: "2 medium onions (8 ounces; 227 g each), finely chopped"
        // The "(8 ounces; 227 g each)" means BOTH measurements are per-onion
        // The trailing "each" applies to all measurements in the parenthetical
        let result = parse_ingredient("2 medium onions (8 ounces; 227 g each), finely chopped");
        println!("{:#?}", result);

        // Should have 3 measurements: primary + 2 alt
        assert_eq!(result.measurements.len(), 3);

        // Primary measurement: 2 medium
        assert_eq!(result.measurements[0].amount, Some("2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("medium".to_string()));

        // Alt measurement 1: 8 ounces each (trailing "each" applies to all)
        assert_eq!(result.measurements[1].amount, Some("8".to_string()));
        assert_eq!(result.measurements[1].unit, Some("ounces each".to_string()));

        // Alt measurement 2: 227 g each
        assert_eq!(result.measurements[2].amount, Some("227".to_string()));
        assert_eq!(result.measurements[2].unit, Some("g each".to_string()));

        // Item should be "onions"
        assert_eq!(result.item, "onions");

        // Note should be "finely chopped"
        assert_eq!(result.note, Some("finely chopped".to_string()));
    }

    #[test]
    fn test_is_prep_note() {
        assert!(is_prep_note("scrubbed clean"));
        assert!(is_prep_note("chopped"));
        assert!(is_prep_note("finely chopped"));
        assert!(!is_prep_note("potatoes"));
    }

    #[test]
    fn test_real_world_samples() {
        // Test a variety of real-world ingredient formats from different sites
        let test_cases = [
            // smittenkitchen.com - parenthetical with weight per item
            (
                "4 (about 8 ounces or 225 grams each) russet potatoes, scrubbed clean",
                "russet potatoes",
                vec![("4", None), ("8", Some("ounces")), ("225", Some("grams"))],
                Some("scrubbed clean"),
            ),
            // cookingclassy.com - double parens (unusual, we accept that the parens stay in item)
            (
                "1 lb Italian Sausage ((casings removed if necessary))",
                "Italian Sausage",
                vec![("1", Some("lb"))],
                None, // Double parens not parsed as note - OK for now
            ),
            // browneyedbaker.com - unicode fractions
            (
                "⅓ cup graham cracker crumbs",
                "graham cracker crumbs",
                vec![("1/3", Some("cup"))],
                None,
            ),
            // mypureplants.com - measurement in parens after
            (
                "⅓ cup Irish whiskey (1 fl oz)",
                "Irish whiskey",
                vec![("1/3", Some("cup")), ("1", Some("fl oz"))],
                None,
            ),
            // acouplecooks.com - "for serving" note
            (
                "Fresh herbs, for serving",
                "Fresh herbs",
                vec![],
                Some("for serving"),
            ),
            // Simple case
            (
                "1/2 cup salted butter (softened)",
                "salted butter",
                vec![("1/2", Some("cup"))],
                Some("softened"),
            ),
        ];

        for (raw, expected_item, _expected_measurements, expected_note) in test_cases {
            let result = parse_ingredient(raw);
            println!("\nRAW: {:?}", raw);
            println!("  => item: {:?}", result.item);
            println!("     measurements: {:?}", result.measurements);
            println!("     note: {:?}", result.note);

            // Check item contains expected substring
            assert!(
                result
                    .item
                    .to_lowercase()
                    .contains(&expected_item.to_lowercase()),
                "Item mismatch for '{}': expected '{}' to contain '{}', got '{}'",
                raw,
                result.item,
                expected_item,
                result.item
            );

            // Check note if expected
            if let Some(expected) = expected_note {
                assert!(
                    result
                        .note
                        .as_ref()
                        .map(|n| n.to_lowercase().contains(&expected.to_lowercase()))
                        .unwrap_or(false),
                    "Note mismatch for '{}': expected '{:?}' to contain '{}', got '{:?}'",
                    raw,
                    result.note,
                    expected,
                    result.note
                );
            }
        }
    }
}
