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
    // Multi-word modifiers first (longer matches take priority)
    "lightly packed",
    "firmly packed",
    "loosely packed",
    "slightly heaped",
    "slightly heaping",
    // Single-word modifiers
    "scant",
    "heaping",
    "heaped",
    "rounded",
    "level",
    "generous",
    "good",
    "packed",
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

/// Decode HTML entities using the html-escape crate.
/// Also handles double-encoded entities like "&amp;#8531;" by decoding twice.
fn decode_html_entities(s: &str) -> String {
    // First pass: decode entities (this handles &amp; -> & among others)
    let decoded = html_escape::decode_html_entities(s);

    // Second pass: decode again to handle double-encoded entities
    // e.g., "&amp;#8531;" -> "&#8531;" -> "⅓"
    let decoded = html_escape::decode_html_entities(&decoded);

    decoded.into_owned()
}

/// Normalize unicode characters to their ASCII equivalents.
/// This handles:
/// - Non-breaking spaces → regular spaces
/// - Unicode fractions (½, ⅓, etc.) → ASCII fractions (1/2, 1/3, etc.)
/// - Unicode dashes (en-dash, em-dash) → ASCII hyphen
fn normalize_unicode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 10);
    let chars: Vec<char> = s.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        match c {
            // Non-breaking space → regular space
            '\u{a0}' => result.push(' '),

            // En-dash and em-dash → ASCII hyphen
            '–' | '—' => result.push('-'),

            // Unicode fractions → ASCII fractions
            // Add space if preceded by a digit (e.g., "1½" -> "1 1/2")
            '½' | '⅓' | '⅔' | '¼' | '¾' | '⅕' | '⅖' | '⅗' | '⅘' | '⅙' | '⅚' | '⅛' | '⅜' | '⅝'
            | '⅞' => {
                let frac = match c {
                    '½' => "1/2",
                    '⅓' => "1/3",
                    '⅔' => "2/3",
                    '¼' => "1/4",
                    '¾' => "3/4",
                    '⅕' => "1/5",
                    '⅖' => "2/5",
                    '⅗' => "3/5",
                    '⅘' => "4/5",
                    '⅙' => "1/6",
                    '⅚' => "5/6",
                    '⅛' => "1/8",
                    '⅜' => "3/8",
                    '⅝' => "5/8",
                    '⅞' => "7/8",
                    _ => unreachable!(),
                };
                if i > 0 && chars[i - 1].is_ascii_digit() {
                    result.push(' ');
                }
                result.push_str(frac);
            }

            // All other characters pass through unchanged
            _ => result.push(c),
        }
    }

    result
}

/// Convert word numbers to digits at the start of the string.
/// Only converts at word boundaries and only at the start to avoid
/// changing words like "someone" or "twenty-one" mid-string.
fn normalize_word_numbers(s: &str) -> String {
    let word_to_digit = [
        ("one", "1"),
        ("two", "2"),
        ("three", "3"),
        ("four", "4"),
        ("five", "5"),
        ("six", "6"),
        ("seven", "7"),
        ("eight", "8"),
        ("nine", "9"),
        ("ten", "10"),
        ("eleven", "11"),
        ("twelve", "12"),
    ];

    let s_lower = s.to_lowercase();
    for (word, digit) in word_to_digit {
        if s_lower.starts_with(word) {
            // Check for word boundary (space or end of string)
            let after = &s[word.len()..];
            if after.is_empty() || after.starts_with(char::is_whitespace) {
                return format!("{}{}", digit, after);
            }
        }
    }
    s.to_string()
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

    // Decode HTML entities and normalize unicode before processing
    let decoded = decode_html_entities(raw);
    let normalized = normalize_unicode(&decoded);
    let mut remaining = normalize_word_numbers(&normalized);
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
                // Also strip trailing comma before the parenthetical (e.g., "onion, (diced)")
                let before = remaining[..start]
                    .trim_end()
                    .trim_end_matches(',')
                    .trim_end();
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
                // Also strip trailing comma before the parenthetical
                let before = remaining[..start]
                    .trim_end()
                    .trim_end_matches(',')
                    .trim_end();
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

    let (mut base_unit, mut after_unit) = extract_unit(&remaining);

    // Step 4a: Handle "N unit container" compound units (e.g., "14 ounce can")
    // If no unit was found, check if remaining starts with a compound unit pattern
    if base_unit.is_none() {
        if let Some((compound_unit, after_compound)) = try_extract_compound_unit(&remaining) {
            base_unit = Some(compound_unit);
            after_unit = after_compound;
        }
    }

    remaining = after_unit;

    // Combine modifiers with unit: prefer pre-unit modifier, fall back to pre-amount modifier
    let modifier = pre_unit_modifier.or(pre_amount_modifier);
    let primary_unit = match (modifier, base_unit) {
        (Some(m), Some(u)) => Some(format!("{} {}", m, u)),
        (Some(m), None) => Some(m), // modifier without unit (rare but possible)
        (None, u) => u,
    };

    // Step 4.5: Handle " or " alternatives in remaining text
    // e.g., remaining = " or 3 heaping cups frozen pineapple"
    // Only split if what follows "or" is a valid measurement (has amount AND unit)
    // This avoids false positives like "vanilla or chocolate ice cream"
    let remaining_trimmed = remaining.trim_start();
    let remaining_lower = remaining_trimmed.to_lowercase();
    if remaining_lower.starts_with("or ") {
        // Find the byte position after "or " (safe since "or " is ASCII)
        let after_or = remaining_trimmed[3..].trim_start();

        // Try to parse as measurement, following the same flow as main parsing:
        // 1. Strip pre-amount modifier (e.g., "scant 1 cup")
        let (or_pre_amount_modifier, after_or_modifier) = strip_measurement_modifier(after_or);

        // 2. Extract amount
        let (or_amount, after_or_amount) = extract_amount(&after_or_modifier);

        // 3. Strip pre-unit modifier (e.g., "3 heaping cups")
        let (or_pre_unit_modifier, after_or_pre_unit) =
            strip_measurement_modifier(&after_or_amount);

        // 4. Extract unit
        let (or_base_unit, after_or_unit) = extract_unit(&after_or_pre_unit);

        // Only treat as alternative if we got BOTH amount AND unit
        if or_amount.is_some() && or_base_unit.is_some() {
            // Combine modifiers with unit (prefer pre-unit, fall back to pre-amount)
            let or_modifier = or_pre_unit_modifier.or(or_pre_amount_modifier);
            let or_unit = match (or_modifier, or_base_unit) {
                (Some(m), Some(u)) => Some(format!("{} {}", m, u)),
                (None, u) => u,
                _ => None,
            };

            alt_measurements.push(Measurement {
                amount: or_amount,
                unit: or_unit,
            });

            remaining = after_or_unit;
        }
    }

    // Step 4.6: Handle " / " alternatives in remaining text
    // e.g., remaining = " / 100g celery root" (after parsing "3.5 ounces")
    // This handles metric/imperial alternatives like "3.5 oz / 100g"
    // Loop to handle multiple: "3/4 cup / 4 oz / 115g toasted sunflower seeds"
    loop {
        let remaining_trimmed = remaining.trim_start();
        if !remaining_trimmed.starts_with("/ ") {
            break;
        }
        let after_slash = remaining_trimmed[2..].trim_start();

        // Try to parse as measurement
        let (slash_pre_amount_modifier, after_slash_modifier) =
            strip_measurement_modifier(after_slash);
        let (slash_amount, after_slash_amount) = extract_amount(&after_slash_modifier);
        let (slash_pre_unit_modifier, after_slash_pre_unit) =
            strip_measurement_modifier(&after_slash_amount);
        let (slash_base_unit, after_slash_unit) = extract_unit(&after_slash_pre_unit);

        // Only treat as alternative if we got BOTH amount AND unit
        if slash_amount.is_some() && slash_base_unit.is_some() {
            let slash_modifier = slash_pre_unit_modifier.or(slash_pre_amount_modifier);
            let slash_unit = match (slash_modifier, slash_base_unit) {
                (Some(m), Some(u)) => Some(format!("{} {}", m, u)),
                (None, u) => u,
                _ => None,
            };

            alt_measurements.push(Measurement {
                amount: slash_amount,
                unit: slash_unit,
            });

            remaining = after_slash_unit;
        } else {
            break;
        }
    }

    // Step 4.7: Handle metric units attached to numbers without separator
    // e.g., remaining = "65g granulated sugar" (after parsing "1/3 cup")
    // This handles sprinklebakes-style "1/3 cup 65g sugar"
    // Also handles "120g/2.75 oz." format with slash-separated alternatives
    // Loop to handle multiple attached measurements, including slash patterns
    let mut found_attached_metric = false;
    loop {
        let remaining_trimmed = remaining.trim_start();

        // Check for slash-separated alternative (e.g., "/8 oz." after "226g")
        // Only do this AFTER we've found at least one attached metric, to avoid
        // false positives like "1/2cup" being parsed as "1" then "/2 cup"
        if found_attached_metric && remaining_trimmed.starts_with('/') {
            let after_slash = remaining_trimmed[1..].trim_start();
            let (slash_amount, after_slash_amount) = extract_amount(after_slash);
            let (slash_unit, after_slash_unit) = extract_unit(&after_slash_amount);

            if slash_amount.is_some() && slash_unit.is_some() {
                alt_measurements.push(Measurement {
                    amount: slash_amount,
                    unit: slash_unit,
                });
                remaining = after_slash_unit;
                continue;
            }
        }

        // Check for attached metric (e.g., "65g" at start of remaining)
        if let Some((attached_measurement, after_attached)) =
            try_extract_attached_metric(&remaining)
        {
            alt_measurements.push(attached_measurement);
            remaining = after_attached;
            found_attached_metric = true;
        } else {
            break;
        }
    }

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

    // Step 5.5: Handle " or " alternatives in the MIDDLE of remaining text
    // e.g., remaining = "fresh dill or 1 teaspoon dried dill"
    // This handles fresh/dried herb alternatives like "1 tbsp fresh basil or 1 tsp dried basil"
    // The text before " or " becomes the item, and "or [measurement] [item]" goes into the note
    // Only apply if note isn't already set
    if note.is_none() {
        if let Some(or_idx) = remaining.to_lowercase().find(" or ") {
            let before_or = remaining[..or_idx].trim();
            let after_or = remaining[or_idx + 4..].trim(); // Skip " or "

            // Try to parse what's after "or" as a measurement
            let (_or_pre_amount_modifier, after_or_modifier) = strip_measurement_modifier(after_or);
            let (or_amount, after_or_amount) = extract_amount(&after_or_modifier);
            let (_or_pre_unit_modifier, after_or_pre_unit) =
                strip_measurement_modifier(&after_or_amount);
            let (or_base_unit, _after_or_unit) = extract_unit(&after_or_pre_unit);

            // Only treat as alternative if we got BOTH amount AND unit
            // This avoids false positives like "vanilla or chocolate ice cream"
            if or_amount.is_some() && or_base_unit.is_some() {
                // Move the alternative to the note and keep just the item before "or"
                note = Some(format!("or {}", after_or));
                remaining = before_or.to_string();
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
    // Strip leading commas that can occur after units (e.g., "2 large, boneless chicken")
    let item = remaining.trim().trim_start_matches(',').trim().to_string();

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

        // Check for "X and Y/Z" pattern: "2 and 1/2"
        if words.len() >= 3
            && words[1].eq_ignore_ascii_case("and")
            && first.chars().all(|c| c.is_ascii_digit())
            && is_fraction(words[2])
        {
            // Normalize to "X Y/Z" format (e.g., "2 1/2")
            let amount = format!("{} {}", first, words[2]);
            let remaining_after_fraction = words[3..].join(" ");
            return (Some(amount), remaining_after_fraction);
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

        // Check for range with "or": "3 or 4" (meaning 3-4, not alternatives)
        // This handles "3 or 4 drops of Tabasco" → amount="3 or 4", unit="drops"
        if words.len() >= 3
            && words[1].eq_ignore_ascii_case("or")
            && is_amount_like(words[0])
            && is_amount_like(words[2])
        {
            let amount = format!("{} or {}", words[0], words[2]);
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

        // Check for hyphenated range with spaces: "1 - 2"
        if words.len() >= 3
            && words[1] == "-"
            && is_amount_like(words[0])
            && is_amount_like(words[2])
        {
            // Normalize to "X-Y" format (no spaces)
            let amount = format!("{}-{}", words[0], words[2]);
            let remaining_after_range = words[3..].join(" ");
            return (Some(amount), remaining_after_range);
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

/// Container types that can form compound units like "14 ounce can"
const CONTAINERS: &[&str] = &[
    "packages", "package", "bottles", "bottle", "boxes", "cans", "jars", "bags", "box", "can",
    "jar", "bag", "pkgs", "pkg",
];

/// Weight/volume units that can precede containers in compound units
const WEIGHT_UNITS_FOR_COMPOUND: &[&str] = &[
    "ounce",
    "ounces",
    "oz",
    "gram",
    "grams",
    "g",
    "pound",
    "pounds",
    "lb",
    "lbs",
    "ml",
    "milliliter",
    "milliliters",
    "liter",
    "liters",
    "l",
];

/// Try to extract a compound unit like "14 ounce can" or "10 oz bag".
/// Also handles hyphenated forms like "28-oz. can" or "14-ounce can".
/// Returns (compound_unit, remaining) if found, None otherwise.
fn try_extract_compound_unit(s: &str) -> Option<(String, String)> {
    let s = s.trim();
    let words: Vec<&str> = s.split_whitespace().collect();

    if words.is_empty() {
        return None;
    }

    // Check for hyphenated form first: "28-oz." or "14-ounce" followed by container
    // Pattern: FIRST_WORD contains hyphen with NUMBER-UNIT format
    let first = words[0];
    if let Some(hyphen_pos) = first.find('-') {
        let before_hyphen = &first[..hyphen_pos];
        let after_hyphen = &first[hyphen_pos + 1..];

        // Before hyphen must be a number
        if is_amount_like(before_hyphen) {
            // After hyphen must be a weight unit (possibly with trailing period)
            let after_lower = after_hyphen.to_lowercase();
            let after_no_dot = after_lower.trim_end_matches('.');
            let is_weight = WEIGHT_UNITS_FOR_COMPOUND.iter().any(|&u| after_no_dot == u);

            if is_weight && words.len() >= 2 {
                // Second word must be a container
                let second_lower = words[1].to_lowercase();
                let is_container = CONTAINERS.iter().any(|&c| second_lower == c);

                if is_container {
                    // Build compound unit preserving original format
                    let compound_unit = format!("{} {}", first, words[1]);
                    let remaining = words[2..].join(" ");
                    return Some((compound_unit, remaining));
                }
            }
        }
    }

    // Fall back to spaced form: NUMBER UNIT CONTAINER
    // Need at least 3 words: NUMBER UNIT CONTAINER
    if words.len() < 3 {
        return None;
    }

    // First word must be a number (integer or decimal)
    if !is_amount_like(first) {
        return None;
    }

    // Second word must be a weight/volume unit
    let second_lower = words[1].to_lowercase();
    let is_weight_unit = WEIGHT_UNITS_FOR_COMPOUND
        .iter()
        .any(|&u| second_lower == u || second_lower == format!("{}.", u));
    if !is_weight_unit {
        return None;
    }

    // Third word must be a container
    let third_lower = words[2].to_lowercase();
    let is_container = CONTAINERS.iter().any(|&c| third_lower == c);
    if !is_container {
        return None;
    }

    // Build the compound unit (preserving original case)
    let compound_unit = format!("{} {} {}", words[0], words[1], words[2]);

    // Calculate remaining string
    let remaining = words[3..].join(" ");

    Some((compound_unit, remaining))
}

/// Metric units that can be attached to numbers without space (e.g., "65g", "100ml")
const ATTACHED_METRIC_UNITS: &[&str] = &["kg", "g", "mg", "ml", "l", "oz", "lb", "lbs"];

/// Try to extract a metric measurement attached to a number at the start of the string.
/// e.g., "65g granulated sugar" -> Some((Measurement{amount: "65", unit: "g"}, "granulated sugar"))
/// Also handles "120g/2.75 oz." format - extracts "120g" and leaves "/2.75 oz." for next iteration.
/// Returns None if no attached metric is found.
fn try_extract_attached_metric(s: &str) -> Option<(Measurement, String)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Must start with a digit
    if !s
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        return None;
    }

    // Find where the number ends and look for attached unit
    let mut num_end = 0;
    let mut has_digit = false;
    let mut has_dot = false;

    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() {
            has_digit = true;
            num_end = i + 1;
        } else if c == '.' && !has_dot && has_digit {
            // Allow one decimal point
            has_dot = true;
            num_end = i + 1;
        } else {
            break;
        }
    }

    if !has_digit || num_end == 0 {
        return None;
    }

    // Check if trailing dot should be excluded (e.g., "65." is not a valid amount by itself
    // unless followed by digits)
    let amount_str = s[..num_end].trim_end_matches('.');
    if amount_str.is_empty() {
        return None;
    }

    // Now check if immediately followed by a metric unit (no space)
    let after_num = &s[num_end..];

    // Check each metric unit (longest first would be ideal, but these are short)
    for &unit in ATTACHED_METRIC_UNITS {
        let after_lower = after_num.to_lowercase();
        if after_lower.starts_with(unit) {
            // Check for word boundary after unit
            let after_unit = &after_num[unit.len()..];
            // Valid boundaries: end of string, space, comma, slash, period (abbreviation)
            if after_unit.is_empty()
                || after_unit.starts_with(char::is_whitespace)
                || after_unit.starts_with(',')
                || after_unit.starts_with('/')
                || after_unit.starts_with('.')
            {
                // Skip any trailing period (abbreviation like "oz.")
                let unit_with_case = &after_num[..unit.len()];
                let remaining = after_unit.trim_start_matches('.').trim_start();

                return Some((
                    Measurement {
                        amount: Some(amount_str.to_string()),
                        unit: Some(unit_with_case.to_string()),
                    },
                    remaining.to_string(),
                ));
            }
        }
    }

    None
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

    #[test]
    fn test_or_alternative() {
        // Test that "or" alternatives are parsed as separate measurements
        let result = parse_ingredient("1 pound or 3 heaping cups frozen pineapple chunks");
        assert_eq!(result.item, "frozen pineapple chunks");
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("pound".to_string()));
        assert_eq!(result.measurements[1].amount, Some("3".to_string()));
        assert_eq!(
            result.measurements[1].unit,
            Some("heaping cups".to_string())
        );
    }

    #[test]
    fn test_or_alternative_not_split_when_no_unit() {
        // "or" should NOT split when what follows has no recognized unit
        // "chocolate ice cream" has no unit, so keep as part of item name
        let result = parse_ingredient("1 cup vanilla or chocolate ice cream");
        assert_eq!(result.item, "vanilla or chocolate ice cream");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_or_alternative_weights() {
        // Test weight-to-weight alternatives
        let result = parse_ingredient("8 ounces or 225 grams cheese");
        assert_eq!(result.item, "cheese");
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("8".to_string()));
        assert_eq!(result.measurements[0].unit, Some("ounces".to_string()));
        assert_eq!(result.measurements[1].amount, Some("225".to_string()));
        assert_eq!(result.measurements[1].unit, Some("grams".to_string()));
    }

    #[test]
    fn test_or_range_not_alternative() {
        // "3 or 4 drops" is a range (3-4 drops), not two alternative measurements
        let result = parse_ingredient("3 or 4 drops of Tabasco sauce");
        assert_eq!(result.item, "Tabasco sauce");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("3 or 4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("drops".to_string()));
    }

    #[test]
    fn test_slash_metric_alternative() {
        // "3.5 oz / 100g" should parse both measurements
        let result = parse_ingredient("3.5 ounces / 100g celery root, peeled");
        assert_eq!(result.item, "celery root");
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("3.5".to_string()));
        assert_eq!(result.measurements[0].unit, Some("ounces".to_string()));
        assert_eq!(result.measurements[1].amount, Some("100".to_string()));
        assert_eq!(result.measurements[1].unit, Some("g".to_string()));
        assert_eq!(result.note, Some("peeled".to_string()));
    }

    #[test]
    fn test_multiple_slash_alternatives() {
        // "3/4 cup / 4 oz / 115g" should parse all three measurements
        let result = parse_ingredient("3/4 cup / 4 oz / 115g toasted sunflower seeds");
        assert_eq!(result.item, "toasted sunflower seeds");
        assert_eq!(result.measurements.len(), 3);
        assert_eq!(result.measurements[0].amount, Some("3/4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
        assert_eq!(result.measurements[1].amount, Some("4".to_string()));
        assert_eq!(result.measurements[1].unit, Some("oz".to_string()));
        assert_eq!(result.measurements[2].amount, Some("115".to_string()));
        assert_eq!(result.measurements[2].unit, Some("g".to_string()));
    }

    #[test]
    fn test_attached_metric_unit() {
        // "1/3 cup 65g sugar" - metric unit attached to number without space
        let result = parse_ingredient("1/3 cup 65g granulated sugar");
        assert_eq!(result.item, "granulated sugar");
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("1/3".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
        assert_eq!(result.measurements[1].amount, Some("65".to_string()));
        assert_eq!(result.measurements[1].unit, Some("g".to_string()));
    }

    #[test]
    fn test_attached_metric_with_slash_alternatives() {
        // "1 cup 226g/8 oz. unsalted butter" - attached metric with slash alternatives
        let result = parse_ingredient("1 cup 226g/8 oz. unsalted butter, softened");
        assert_eq!(result.item, "unsalted butter");
        assert_eq!(result.measurements.len(), 3);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
        assert_eq!(result.measurements[1].amount, Some("226".to_string()));
        assert_eq!(result.measurements[1].unit, Some("g".to_string()));
        assert_eq!(result.measurements[2].amount, Some("8".to_string()));
        assert_eq!(result.measurements[2].unit, Some("oz".to_string()));
        assert_eq!(result.note, Some("softened".to_string()));
    }

    #[test]
    fn test_fat_ratio_not_attached_metric() {
        // "80/20 ground beef" - the 80/20 is a fat ratio, not a measurement
        let result = parse_ingredient("1 pound 80/20 ground beef");
        assert_eq!(result.item, "80/20 ground beef");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("pound".to_string()));
    }
}
