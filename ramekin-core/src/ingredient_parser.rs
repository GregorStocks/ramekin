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

/// Measurement modifiers that appear before amounts or between amounts and units.
/// These are stripped during parsing but preserved in the raw field.
/// Examples: "scant 1 teaspoon", "2 heaping tablespoons", "1 generous cup"
const MEASUREMENT_MODIFIERS: &[&str] = &[
    "scant",
    "heaping",
    "heaped",
    "rounded",
    "level",
    "generous",
    "packed",
    "lightly packed",
    "firmly packed",
    "loosely packed",
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

/// Strip measurement modifiers from the beginning of a string.
/// Returns (modifier if found, remaining_string).
fn strip_measurement_modifier(s: &str) -> (Option<String>, String) {
    let s_lower = s.to_lowercase();
    let s_trimmed = s.trim();

    for &modifier in MEASUREMENT_MODIFIERS {
        if s_lower.trim().starts_with(modifier) {
            let after = &s_trimmed[modifier.len()..];
            // Make sure it's a word boundary (followed by space or end)
            if after.is_empty() || after.starts_with(char::is_whitespace) {
                return (Some(modifier.to_string()), after.trim().to_string());
            }
        }
    }

    (None, s_trimmed.to_string())
}

/// Decode common HTML entities.
fn decode_html_entities(s: &str) -> String {
    s
        // Named entities for fractions
        .replace("&frac14;", "1/4")
        .replace("&frac12;", "1/2")
        .replace("&frac34;", "3/4")
        // Ampersand-encoded versions (double encoded)
        .replace("&amp;frac14;", "1/4")
        .replace("&amp;frac12;", "1/2")
        .replace("&amp;frac34;", "3/4")
        // Non-breaking space
        .replace("&nbsp;", " ")
        .replace("&amp;nbsp;", " ")
        // Apostrophe / single quote
        .replace("&#39;", "'")
        .replace("&amp;#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        // Ampersand
        .replace("&amp;", "&")
        // Quotes
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        // Less/greater than
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

/// Convert unicode fractions to ASCII equivalents.
/// Adds a space before the fraction if preceded by a digit (e.g., "1½" -> "1 1/2").
fn normalize_unicode_fractions(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 10);
    let chars: Vec<char> = s.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        let fraction = match c {
            '½' => Some("1/2"),
            '⅓' => Some("1/3"),
            '⅔' => Some("2/3"),
            '¼' => Some("1/4"),
            '¾' => Some("3/4"),
            '⅕' => Some("1/5"),
            '⅖' => Some("2/5"),
            '⅗' => Some("3/5"),
            '⅘' => Some("4/5"),
            '⅙' => Some("1/6"),
            '⅚' => Some("5/6"),
            '⅛' => Some("1/8"),
            '⅜' => Some("3/8"),
            '⅝' => Some("5/8"),
            '⅞' => Some("7/8"),
            _ => None,
        };

        if let Some(frac) = fraction {
            // Add space if preceded by a digit
            if i > 0 && chars[i - 1].is_ascii_digit() {
                result.push(' ');
            }
            result.push_str(frac);
        } else {
            result.push(c);
        }
    }

    result
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

    // Decode HTML entities and normalize unicode fractions before processing
    let decoded = decode_html_entities(raw);
    let mut remaining = normalize_unicode_fractions(&decoded);
    let mut measurements = Vec::new();
    let mut note = None;

    // Issue 5: Normalize double parentheses to single
    // e.g., "((about 4 cloves))" -> "(about 4 cloves)"
    while remaining.contains("((") {
        remaining = remaining.replace("((", "(");
    }
    while remaining.contains("))") {
        remaining = remaining.replace("))", ")");
    }

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

    // Step 2: Handle "plus" pattern - extract the "plus X" portion as note
    // e.g., "1 cup flour, plus 2 tablespoons" -> note = "plus 2 tablespoons"
    // Look for ", plus " or just " plus " with a measurement after
    if let Some(plus_idx) = remaining.to_lowercase().find(", plus ") {
        let plus_part = remaining[plus_idx + 2..].trim(); // Skip ", "
        if note.is_none() {
            note = Some(plus_part.to_string());
        }
        remaining = remaining[..plus_idx].trim().to_string();
    } else if let Some(plus_idx) = remaining.to_lowercase().find(" plus ") {
        // Check if there's a measurement-like thing after "plus"
        let after_plus = remaining[plus_idx + 6..].trim();
        if !after_plus.is_empty() {
            // Extract just the "plus ..." part
            let plus_part = remaining[plus_idx + 1..].trim();
            if note.is_none() {
                note = Some(plus_part.to_string());
            }
            remaining = remaining[..plus_idx].trim().to_string();
        }
    }

    // Step 3: Strip measurement modifiers before amount, preserve for unit
    // Handles "scant 1 teaspoon" - modifier goes on the unit as "scant teaspoon"
    let (pre_amount_modifier, after_modifier) = strip_measurement_modifier(&remaining);
    remaining = after_modifier;

    let (primary_amount, after_amount) = extract_amount(&remaining);
    remaining = after_amount;

    // Step 4: Strip measurement modifiers before unit, combine with any pre-amount modifier
    // Handles "2 heaping tablespoons" - modifier goes on the unit as "heaping tablespoons"
    let (pre_unit_modifier, after_modifier) = strip_measurement_modifier(&remaining);
    remaining = after_modifier;

    let (base_unit, after_unit) = extract_unit(&remaining);
    remaining = after_unit;

    // Combine modifiers with unit: prefer pre-unit modifier, fall back to pre-amount modifier
    let modifier = pre_unit_modifier.or(pre_amount_modifier);
    let primary_unit = match (modifier, base_unit) {
        (Some(m), Some(u)) => Some(format!("{} {}", m, u)),
        (Some(m), None) => Some(m), // modifier without unit (rare but possible)
        (None, u) => u,
    };

    // Step 5: Extract note from the end (after comma), if not already set
    if note.is_none() {
        if let Some(comma_idx) = remaining.rfind(',') {
            let potential_note = remaining[comma_idx + 1..].trim();
            // Check if it looks like a prep note
            if is_prep_note(potential_note) {
                note = Some(potential_note.to_string());
                remaining = remaining[..comma_idx].trim().to_string();
            }
        }
    }

    // Step 6: Build measurements list
    if primary_amount.is_some() || primary_unit.is_some() {
        measurements.push(Measurement {
            amount: primary_amount,
            unit: primary_unit,
        });
    }
    measurements.extend(alt_measurements);

    // Step 7: The remaining text is the ingredient item
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
/// Handles ranges like "1 to 4" or "6 to 8" as well as simple amounts.
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

        // Check for range: "1 to 4" or "6 to 8"
        if words.len() >= 3
            && words[1].eq_ignore_ascii_case("to")
            && is_amount_like(words[0])
            && is_amount_like(words[2])
        {
            let amount = format!("{} to {}", words[0], words[2]);
            // Find where the third word ends
            let remaining_after_range = words[3..].join(" ");
            return (Some(amount), remaining_after_range);
        }

        // Check for hyphenated range: "6-8"
        if first.contains('-') && !first.starts_with('-') {
            let parts: Vec<&str> = first.split('-').collect();
            if parts.len() == 2 && is_amount_like(parts[0]) && is_amount_like(parts[1]) {
                let remaining = words[1..].join(" ");
                return (Some(first.to_string()), remaining);
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

/// Check if a string looks like an amount (number, fraction, decimal)
fn is_amount_like(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    // Simple number
    if s.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    // Fraction
    if is_fraction(s) {
        return true;
    }
    // Decimal
    let mut has_digit = false;
    let mut has_dot = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            has_digit = true;
        } else if c == '.' && !has_dot {
            has_dot = true;
        } else {
            return false;
        }
    }
    has_digit
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
                let mut remaining = after.trim_start_matches('.').trim();

                // Strip "of " if present after the unit (e.g., "cloves of garlic" -> "garlic")
                let remaining_lower = remaining.to_lowercase();
                if remaining_lower.starts_with("of ") || remaining_lower == "of" {
                    remaining = remaining[2..].trim_start();
                }

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

    // =========================================================================
    // Issue-specific tests for bugs found in pipeline-test analysis
    // =========================================================================

    #[test]
    fn test_issue1_unicode_fraction_spacing() {
        // Issue: "1½ cups" becomes "11/2 cups" instead of "1 1/2 cups"
        // The unicode fraction normalization concatenates without space

        let result = parse_ingredient("1½ cups all-purpose flour");
        println!("{:#?}", result);
        assert_eq!(result.measurements[0].amount, Some("1 1/2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cups".to_string()));

        let result = parse_ingredient("2¾ cups sugar");
        println!("{:#?}", result);
        assert_eq!(result.measurements[0].amount, Some("2 3/4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cups".to_string()));

        // Standalone fraction should still work
        let result = parse_ingredient("½ teaspoon salt");
        println!("{:#?}", result);
        assert_eq!(result.measurements[0].amount, Some("1/2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("teaspoon".to_string()));
    }

    #[test]
    fn test_issue2_html_entities() {
        // Issue: HTML entities like &amp;frac14;, &nbsp;, &#39; appearing in output
        // These should be decoded before parsing

        let result = parse_ingredient("&frac14; cup fresh cilantro leaves");
        println!("{:#?}", result);
        assert_eq!(result.measurements[0].amount, Some("1/4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
        assert_eq!(result.item, "fresh cilantro leaves");

        let result = parse_ingredient("1&nbsp;tablespoon olive oil");
        println!("{:#?}", result);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("tablespoon".to_string()));

        let result = parse_ingredient("Chef&#39;s Choice Matcha");
        println!("{:#?}", result);
        assert!(result.item.contains("Chef's Choice"));
    }

    #[test]
    fn test_issue3_ranges_with_to() {
        // Issue: "6 to 8 ounces" extracted as just "6", losing the range
        // Should preserve ranges like "6 to 8" or "6-8"

        let result =
            parse_ingredient("4 boneless chicken breasts (about 6 to 8 ounces each), sliced");
        println!("{:#?}", result);
        // Primary measurement should be 4
        assert_eq!(result.measurements[0].amount, Some("4".to_string()));
        // Alt measurement should preserve the range "6 to 8" or at minimum "6-8"
        assert!(result.measurements.len() >= 2);
        let alt_amount = result.measurements[1].amount.as_ref().unwrap();
        assert!(
            alt_amount.contains("6") && (alt_amount.contains("8") || alt_amount.contains("to")),
            "Expected range like '6 to 8' or '6-8', got '{}'",
            alt_amount
        );

        let result = parse_ingredient("1 to 4 medium Russet potatoes");
        println!("{:#?}", result);
        // Should preserve "1 to 4" as a range
        let amount = result.measurements[0].amount.as_ref().unwrap();
        assert!(
            amount.contains("1") && (amount.contains("4") || amount.contains("to")),
            "Expected range, got '{}'",
            amount
        );
    }

    #[test]
    fn test_issue4_parenthetical_size_format() {
        // Issue: "2 (15.5-ounce) cans chickpeas" parses incorrectly
        // The size in parentheses should attach to the unit "cans"

        let result = parse_ingredient("2 (15.5-ounce) cans chickpeas");
        println!("{:#?}", result);
        assert_eq!(result.item, "chickpeas");
        // Should have primary measurement of "2 cans" with alt of "15.5 ounces"
        // or combined "2 15.5-ounce cans"
        assert!(result.measurements.len() >= 1);
        // Check that we got meaningful unit (not just "chickpeas" or empty)
        let has_cans = result
            .measurements
            .iter()
            .any(|m| m.unit.as_ref().map(|u| u.contains("can")).unwrap_or(false));
        assert!(has_cans, "Should have 'cans' as a unit");

        let result =
            parse_ingredient("1 pound cream cheese (2 8-ounce/227-gram packages), softened");
        println!("{:#?}", result);
        assert_eq!(result.item, "cream cheese");
        // Primary should be "1 pound"
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("pound".to_string()));
    }

    #[test]
    fn test_issue5_double_parentheses() {
        // Issue: Double parentheses like "((about 4 cloves))" cause mismatched brackets

        let result = parse_ingredient("1 1/2 Tbsp minced garlic ((about 4 cloves))");
        println!("{:#?}", result);
        assert_eq!(result.measurements[0].amount, Some("1 1/2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("tbsp".to_string()));
        assert_eq!(result.item, "minced garlic");
        // Note should not have unbalanced parentheses
        if let Some(note) = &result.note {
            let open = note.chars().filter(|&c| c == '(').count();
            let close = note.chars().filter(|&c| c == ')').count();
            assert_eq!(open, close, "Unbalanced parentheses in note: '{}'", note);
        }

        let result = parse_ingredient("2.5 oz. fresh spinach ((2 1/2 cups packed))");
        println!("{:#?}", result);
        assert_eq!(result.item, "fresh spinach");
        // Should not have mismatched brackets in output
        assert!(
            !result.item.contains('[') && !result.item.contains(']'),
            "Item should not contain brackets: '{}'",
            result.item
        );
    }

    #[test]
    fn test_issue6_plus_amounts() {
        // Issue: "1 cup flour, plus 2 tablespoons" - second quantity context lost

        let result = parse_ingredient(
            "1 cup all-purpose flour (4 1/2 ounces; 128 g), plus 1 1/2 tablespoons (12 g)",
        );
        println!("{:#?}", result);
        // Should capture the main measurement
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
        // The "plus" part should be captured somehow (in note, or as additional measurement)
        // At minimum, we shouldn't lose the context entirely
        let has_plus_info = result
            .note
            .as_ref()
            .map(|n| n.contains("plus") || n.contains("1 1/2 tablespoons"))
            .unwrap_or(false)
            || result.measurements.iter().any(|m| {
                m.amount.as_ref().map(|a| a == "1 1/2").unwrap_or(false)
                    && m.unit
                        .as_ref()
                        .map(|u| u.contains("tablespoon"))
                        .unwrap_or(false)
            });
        assert!(
            has_plus_info,
            "Should preserve 'plus 1 1/2 tablespoons' info somewhere"
        );

        let result = parse_ingredient("2 tablespoons butter, plus 1 tablespoon chilled");
        println!("{:#?}", result);
        // Primary should be "2 tablespoons"
        assert_eq!(result.measurements[0].amount, Some("2".to_string()));
        // "plus 1 tablespoon chilled" should be in note
        assert!(
            result
                .note
                .as_ref()
                .map(|n| n.contains("plus") || n.contains("1 tablespoon"))
                .unwrap_or(false),
            "Note should contain 'plus' info: {:?}",
            result.note
        );
    }
}
