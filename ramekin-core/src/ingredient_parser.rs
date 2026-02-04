//! Ingredient parsing module.
//!
//! Parses raw ingredient strings (e.g., "2 cups flour, sifted") into structured data.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
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
    /// Section name for grouping (e.g., "For the sauce", "For the dough")
    pub section: Option<String>,
}

/// Common cooking units (lowercase for matching).
/// Sorted by length at runtime (longest first) to avoid partial matches
/// (e.g., "tablespoons" must match before "tb").
static UNITS_SORTED: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut units = UNITS_RAW.to_vec();
    units.sort_by_key(|u| std::cmp::Reverse(u.len()));
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

/// Map of unit variations to their canonical forms.
/// Used by normalize_unit() to standardize units after parsing.
static UNIT_CANONICAL_MAP: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut map = HashMap::new();

    // Volume - small
    map.insert("teaspoon", "tsp");
    map.insert("teaspoons", "tsp");
    map.insert("ts", "tsp");

    map.insert("tablespoon", "tbsp");
    map.insert("tablespoons", "tbsp");
    map.insert("tbs", "tbsp");
    map.insert("tb", "tbsp");

    // Volume - cups
    map.insert("cups", "cup");
    map.insert("c", "cup");

    // Volume - larger
    map.insert("pints", "pint");
    map.insert("pt", "pint");

    map.insert("quarts", "quart");
    map.insert("qt", "quart");

    map.insert("gallons", "gallon");
    map.insert("gal", "gallon");

    map.insert("fluid ounce", "fl oz");
    map.insert("fluid ounces", "fl oz");
    map.insert("fl. oz", "fl oz");

    // Volume - metric
    map.insert("milliliter", "ml");
    map.insert("milliliters", "ml");

    map.insert("liter", "l");
    map.insert("liters", "l");
    map.insert("litre", "l");
    map.insert("litres", "l");

    // Weight - US
    map.insert("ounce", "oz");
    map.insert("ounces", "oz");

    map.insert("pound", "lb");
    map.insert("pounds", "lb");
    map.insert("lbs", "lb");

    // Weight - metric
    map.insert("gram", "g");
    map.insert("grams", "g");

    map.insert("kilogram", "kg");
    map.insert("kilograms", "kg");

    map.insert("milligram", "mg");
    map.insert("milligrams", "mg");

    // Count/Container - normalize plurals to singular
    map.insert("cloves", "clove");
    map.insert("slices", "slice");
    map.insert("pieces", "piece");
    map.insert("pc", "piece");
    map.insert("pcs", "piece");
    map.insert("cans", "can");
    map.insert("jars", "jar");
    map.insert("bottles", "bottle");
    map.insert("bags", "bag");
    map.insert("boxes", "box");
    map.insert("packages", "package");
    map.insert("pkg", "package");
    map.insert("pkgs", "package");
    map.insert("sticks", "stick");
    map.insert("bunches", "bunch");
    map.insert("sprigs", "sprig");
    map.insert("pinches", "pinch");
    map.insert("dashes", "dash");
    map.insert("drops", "drop");
    map.insert("heads", "head");
    map.insert("stalks", "stalk");
    map.insert("handfuls", "handful");
    map.insert("cubes", "cube");

    // Size - normalize xl
    map.insert("xl", "extra-large");

    map
});

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
            if let Some(after) = s_trimmed.get(modifier.len()..) {
                // Make sure it's a word boundary (followed by space or end)
                if after.is_empty() || after.starts_with(char::is_whitespace) {
                    return (Some(modifier.to_string()), after.trim().to_string());
                }
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
    // Handle fractional words first (before whole numbers)
    let fraction_to_digit = [("half", "1/2"), ("quarter", "1/4")];

    let s_lower = s.to_lowercase();
    for (word, digit) in fraction_to_digit {
        if s_lower.starts_with(word) {
            if let Some(after) = s.get(word.len()..) {
                if after.is_empty() || after.starts_with(char::is_whitespace) {
                    return format!("{}{}", digit, after);
                }
            }
        }
    }

    // Handle whole number words
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

    for (word, digit) in word_to_digit {
        if s_lower.starts_with(word) {
            // Check for word boundary (space or end of string)
            if let Some(after) = s.get(word.len()..) {
                if after.is_empty() || after.starts_with(char::is_whitespace) {
                    return format!("{}{}", digit, after);
                }
            }
        }
    }
    s.to_string()
}

fn strip_leading_list_marker(s: &str) -> String {
    let mut remaining = s.trim_start();
    loop {
        let mut chars = remaining.chars();
        let Some(first) = chars.next() else {
            break;
        };
        if matches!(first, '-' | '+' | '*' | '&') {
            let rest = chars.as_str();
            let rest_first = rest.chars().next();
            let should_strip = matches!(
                rest_first,
                Some(c) if c.is_ascii_digit() || c.is_whitespace() || c == '('
            );
            if should_strip {
                remaining = rest.trim_start();
                continue;
            }
        }
        break;
    }
    remaining.to_string()
}

/// Insert space between digits and letters that are clearly separate words.
/// Handles cases like "1finely" → "1 finely" and "450gpowdered" → "450g powdered"
/// But preserves dimension patterns like "6x6-inch".
fn normalize_digit_letter_spacing(s: &str) -> String {
    // Step 1: Handle unit words like "grams" attached to numbers
    // "450grams" → "450 grams" (insert space before the whole unit word)
    let re_unit_word = Regex::new(r"(?i)(\d+)(grams?)\b").unwrap();
    let s = re_unit_word.replace_all(s, "$1 $2");

    // Step 2: Handle "g" metric unit followed by other letters
    // "450gpowdered" → "450g powdered"
    let re_metric_g = Regex::new(r"(?i)(\d+g)([a-z])").unwrap();
    let s = re_metric_g.replace_all(&s, "$1 $2");

    // Step 3: Handle digit(s) followed by 4+ letters (clearly a word)
    // "1finely" → "1 finely"
    let re_digit_word = Regex::new(r"(?i)(\d+)([a-z]{4,})").unwrap();
    let s = re_digit_word.replace_all(&s, "$1 $2");

    s.into_owned()
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
            section: None,
        };
    }

    // Decode HTML entities and normalize unicode before processing
    let decoded = decode_html_entities(raw);
    let normalized = normalize_unicode(&decoded);
    let normalized = strip_leading_list_marker(&normalized);
    let normalized = normalize_digit_letter_spacing(&normalized);
    let mut remaining = normalize_word_numbers(&normalized);
    let mut measurements = Vec::new();
    let mut note = None;

    // Strip "Optional:" or "Optional -" prefix, capturing for note
    let mut optional_prefix = false;
    let remaining_lower = remaining.to_lowercase();
    if remaining_lower.starts_with("optional:") {
        remaining = remaining.get(9..).unwrap_or("").trim().to_string();
        optional_prefix = true;
    } else if remaining_lower.starts_with("optional -") {
        remaining = remaining.get(10..).unwrap_or("").trim().to_string();
        optional_prefix = true;
    } else if remaining_lower.starts_with("optional-") {
        remaining = remaining.get(9..).unwrap_or("").trim().to_string();
        optional_prefix = true;
    }

    // Issue 5: Normalize double parentheses to single
    // e.g., "((about 4 cloves))" -> "(about 4 cloves)"
    while remaining.contains("((") {
        remaining = remaining.replace("((", "(");
    }
    while remaining.contains("))") {
        remaining = remaining.replace("))", ")");
    }

    // Unwrap leading parentheticals that contain quantities
    // e.g., "(half stick) butter" -> "1/2 stick butter"
    // But NOT "(optional) 1/4 cup" which should keep the paren structure
    if remaining.starts_with('(') {
        if let Some(close_idx) = remaining.find(')') {
            let paren_content = remaining.get(1..close_idx).unwrap_or("").trim();

            // Normalize word numbers in paren content (e.g., "half" -> "1/2", "two" -> "2")
            let normalized_content = normalize_word_numbers(paren_content);

            // Only unwrap if the content starts with a digit (after normalization)
            // or is a known measurement modifier like "heaping", "scant"
            let first_char = normalized_content.chars().next();
            let is_quantity = first_char.is_some_and(|c| c.is_ascii_digit());
            let is_modifier = MEASUREMENT_MODIFIERS
                .iter()
                .any(|&m| normalized_content.eq_ignore_ascii_case(m));

            if is_quantity || is_modifier {
                let after_paren = remaining.get(close_idx + 1..).unwrap_or("").trim();
                remaining = if after_paren.is_empty() {
                    normalized_content
                } else {
                    format!("{} {}", normalized_content, after_paren)
                };
            }
        }
    }

    // Step 1: Extract any parenthetical content (measurements or prep notes)
    // e.g., "1 stick (113g) butter" -> extract "(113g)" as alt measurement
    // e.g., "1/2 cup butter (softened)" -> extract "(softened)" as note
    let mut alt_measurements = Vec::new();
    while let Some(start) = remaining.find('(') {
        // Find ')' in the substring after '('
        let after_open = match remaining.get(start..) {
            Some(s) => s,
            None => break,
        };
        let Some(end_offset) = after_open.find(')') else {
            break;
        };
        // end_offset is relative to start, so absolute close paren is at start + end_offset
        let paren_content = match remaining.get(start + 1..start + end_offset) {
            Some(s) => s,
            None => break,
        };

        // First check if this is a prep note (like "softened", "chopped", etc.)
        if is_prep_note(paren_content) && note.is_none() {
            // Strip leading comma (e.g., from raw like "tomato (, sliced)")
            note = Some(
                paren_content
                    .trim()
                    .trim_start_matches(',')
                    .trim()
                    .to_string(),
            );
            // Remove the parenthetical from remaining
            // Also strip trailing comma before the parenthetical (e.g., "onion, (diced)")
            let before = remaining
                .get(..start)
                .unwrap_or("")
                .trim_end()
                .trim_end_matches(',')
                .trim_end();
            let after = remaining
                .get(start + end_offset + 1..)
                .unwrap_or("")
                .trim_start();
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
            let before = remaining
                .get(..start)
                .unwrap_or("")
                .trim_end()
                .trim_end_matches(',')
                .trim_end();
            let after = remaining
                .get(start + end_offset + 1..)
                .unwrap_or("")
                .trim_start();
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
    }

    // Step 2: Handle "plus" pattern - extract the "plus X" portion as note
    // e.g., "1 cup flour, plus 2 tablespoons" -> note = "plus 2 tablespoons"
    // Look for ", plus " or just " plus " with a measurement after
    if let Some(plus_idx) = remaining.to_lowercase().find(", plus ") {
        // Skip ", " to get "plus ..."
        if let Some(plus_part) = remaining.get(plus_idx + 2..) {
            if note.is_none() {
                note = Some(plus_part.trim().to_string());
            }
        }
        remaining = remaining.get(..plus_idx).unwrap_or("").trim().to_string();
    } else if let Some(plus_idx) = remaining.to_lowercase().find(" plus ") {
        // Check if there's a measurement-like thing after "plus"
        let after_plus = remaining.get(plus_idx + 6..).unwrap_or("").trim();
        if !after_plus.is_empty() {
            // Extract just the "plus ..." part (skip the leading space)
            if let Some(plus_part) = remaining.get(plus_idx + 1..) {
                if note.is_none() {
                    note = Some(plus_part.trim().to_string());
                }
            }
            remaining = remaining.get(..plus_idx).unwrap_or("").trim().to_string();
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
        // Use the length of what was stripped to get from original (preserving case)
        let after_or = remaining_trimmed.get(3..).unwrap_or("").trim_start();

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
        let Some(after_slash) = remaining_trimmed.strip_prefix("/ ") else {
            break;
        };
        let after_slash = after_slash.trim_start();

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
        if found_attached_metric {
            if let Some(after_slash) = remaining_trimmed.strip_prefix('/') {
                let after_slash = after_slash.trim_start();
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
            if let Some(potential_note) = remaining.get(comma_idx + 1..) {
                let potential_note = potential_note.trim();
                // Check if it looks like a prep note
                if is_prep_note(potential_note) {
                    note = Some(potential_note.to_string());
                    remaining = remaining.get(..comma_idx).unwrap_or("").trim().to_string();
                }
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
            let before_or = remaining.get(..or_idx).unwrap_or("").trim();
            let after_or = remaining.get(or_idx + 4..).unwrap_or("").trim(); // Skip " or "

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

    // Step 6.5: Normalize all measurement units to canonical forms
    for m in &mut measurements {
        if let Some(ref unit) = m.unit {
            m.unit = Some(normalize_unit(unit));
        }
    }

    // Step 7: The remaining text is the ingredient item
    // Strip leading commas that can occur after units (e.g., "2 large, boneless chicken")
    // Strip trailing " )" that can occur from double-paren patterns like "((45ml) )"
    // Strip trailing commas (e.g., "pork tenderloins,")
    // Normalize " ," to "," (space before comma from parenthetical extraction)
    let item = remaining
        .trim()
        .trim_start_matches(',')
        .trim()
        .trim_end_matches(" )")
        .trim()
        .trim_end_matches(',')
        .trim()
        .replace(" ,", ",")
        .to_string();

    // Prepend "optional" to note if we stripped that prefix
    if optional_prefix {
        note = match note {
            Some(n) => Some(format!("optional, {}", n)),
            None => Some("optional".to_string()),
        };
    }

    // If we didn't extract anything useful, just use raw as item
    if item.is_empty() && measurements.is_empty() {
        return ParsedIngredient {
            item: raw.to_string(),
            measurements: vec![],
            note: None,
            raw: Some(raw.to_string()),
            section: None,
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
        section: None,
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
    for part in normalized.split([';', ',']) {
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
            result = result.get(..idx).unwrap_or("").to_string();
        }
    }

    // Also handle qualifiers at the start
    let start_qualifiers = ["about ", "approximately ", "approx ", "roughly ", "~"];
    let lower = result.to_lowercase();
    for q in start_qualifiers {
        if lower.starts_with(q) {
            result = result.get(q.len()..).unwrap_or("").to_string();
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

        // Check for mixed number range first: "2 1/2 - 4 1/2" or "2 1/2 - 3"
        // Pattern: digit, fraction, "-", digit (optionally followed by fraction)
        if words.len() >= 4
            && first.chars().all(|c| c.is_ascii_digit())
            && is_fraction(second)
            && words[2] == "-"
            && is_amount_like(words[3])
        {
            // Could be "2 1/2 - 4 1/2" (5+ words) or "2 1/2 - 3" (4+ words)
            let (second_amount, remaining_start) = if words.len() >= 5 && is_fraction(words[4]) {
                (format!("{} {}", words[3], words[4]), 5)
            } else {
                (words[3].to_string(), 4)
            };
            let amount = format!("{} {}-{}", first, second, second_amount);
            let remaining = words[remaining_start..].join(" ");
            return (Some(amount), remaining);
        }

        // Check if first is a whole number and second is a fraction
        if first.chars().all(|c| c.is_ascii_digit()) && is_fraction(second) {
            let amount = format!("{} {}", first, second);
            // Find where the second word ends in the original string
            if let Some(pos) = s.find(second) {
                let end_pos = pos + second.len();
                return (
                    Some(amount),
                    s.get(end_pos..).unwrap_or("").trim().to_string(),
                );
            }
        }

        // Check for "X and Y/Z" or "X & Y/Z" pattern: "2 and 1/2" or "1 & 1/2"
        if words.len() >= 3
            && (words[1].eq_ignore_ascii_case("and") || words[1] == "&")
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
                s.get(word_len..).unwrap_or("").trim().to_string(),
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
    if let Some((before, after)) = s.split_once('/') {
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
            let after = s.get(unit.len()..).unwrap_or("");
            if after.is_empty()
                || after.starts_with(|c: char| c.is_whitespace() || c == '.' || c == ',')
            {
                // Skip any trailing period or whitespace
                let mut remaining = after.trim_start_matches('.').trim();

                // Strip "of " if present after the unit (e.g., "cloves of garlic" -> "garlic")
                let remaining_lower = remaining.to_lowercase();
                if remaining_lower.starts_with("of ") || remaining_lower == "of" {
                    remaining = remaining.get(2..).unwrap_or("").trim_start();
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
    if let Some((before_hyphen, after_hyphen)) = first.split_once('-') {
        // Before hyphen must be a number
        if is_amount_like(before_hyphen) {
            // After hyphen must be a weight unit (possibly with trailing period)
            let after_lower = after_hyphen.to_lowercase();
            let after_no_dot = after_lower.trim_end_matches('.');
            let is_weight = WEIGHT_UNITS_FOR_COMPOUND.contains(&after_no_dot);

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
    let amount_str = s.get(..num_end)?.trim_end_matches('.');
    if amount_str.is_empty() {
        return None;
    }

    // Now check if immediately followed by a metric unit (no space)
    let after_num = s.get(num_end..)?;

    // Check each metric unit (longest first would be ideal, but these are short)
    for &unit in ATTACHED_METRIC_UNITS {
        let after_lower = after_num.to_lowercase();
        if after_lower.starts_with(unit) {
            // Check for word boundary after unit
            let after_unit = after_num.get(unit.len()..).unwrap_or("");
            // Valid boundaries: end of string, space, comma, slash, period (abbreviation)
            if after_unit.is_empty()
                || after_unit.starts_with(char::is_whitespace)
                || after_unit.starts_with(',')
                || after_unit.starts_with('/')
                || after_unit.starts_with('.')
            {
                // Skip any trailing period (abbreviation like "oz.")
                let unit_with_case = after_num.get(..unit.len()).unwrap_or("");
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

/// Normalize a unit string to its canonical form.
///
/// Handles:
/// - Direct mappings: "cups" → "cup", "tablespoons" → "tbsp"
/// - Modifiers: "heaping cups" → "heaping cup"
/// - "each" suffix: "ounces each" → "oz each"
///
/// Returns the original unit if no normalization is needed.
fn normalize_unit(unit: &str) -> String {
    let unit = unit.trim();
    if unit.is_empty() {
        return unit.to_string();
    }

    // Check for "each" suffix first (case-insensitive check, preserve original for base)
    let (base_unit, each_suffix) = if unit.to_lowercase().ends_with(" each") {
        // Safe because " each" is 5 ASCII bytes
        (unit.get(..unit.len() - 5).unwrap_or(unit), " each")
    } else {
        (unit, "")
    };

    // Check if there's a modifier prefix (from MEASUREMENT_MODIFIERS)
    let base_lower = base_unit.to_lowercase();
    for &modifier in MEASUREMENT_MODIFIERS {
        if base_lower.starts_with(modifier) {
            let after_modifier = base_unit.get(modifier.len()..).unwrap_or("").trim();
            if !after_modifier.is_empty() {
                // Normalize the unit part after the modifier
                let normalized_base = normalize_unit_base(after_modifier);
                return format!("{} {}{}", modifier, normalized_base, each_suffix);
            }
        }
    }

    // No modifier, just normalize the base unit
    let normalized = normalize_unit_base(base_unit);
    format!("{}{}", normalized, each_suffix)
}

/// Normalize a base unit (without modifiers) using UNIT_CANONICAL_MAP.
fn normalize_unit_base(unit: &str) -> String {
    let unit_lower = unit.to_lowercase();
    if let Some(&canonical) = UNIT_CANONICAL_MAP.get(unit_lower.as_str()) {
        canonical.to_string()
    } else {
        unit.to_string()
    }
}

/// Lines that should be completely ignored (scraper artifacts, not ingredients or headers).
/// These are checked case-insensitively.
const IGNORED_LINE_PATTERNS: &[&str] = &[
    "gather your ingredients",
    "gather the ingredients",
    "here's what you'll need",
    "here's what you need",
    "what you'll need",
    "what you need",
    "you will need",
    "you'll need",
    "ingredients list",
];

/// Prefixes that indicate a line should be ignored (not an ingredient).
const IGNORED_LINE_PREFIXES: &[&str] = &[
    "special equipment:",
    "equipment:",
    "tools:",
    "notes:",
    "note:",
    "tip:",
    "tips:",
];

/// Check if a line should be completely ignored (scraper artifact).
/// Returns true if the line should be skipped entirely.
pub fn should_ignore_line(raw: &str) -> bool {
    let trimmed = raw.trim();
    let lower = trimmed.to_lowercase();

    // Check exact matches (case-insensitive)
    for &pattern in IGNORED_LINE_PATTERNS {
        if lower == pattern {
            return true;
        }
    }

    // Check prefixes (case-insensitive)
    for &prefix in IGNORED_LINE_PREFIXES {
        if lower.starts_with(prefix) {
            return true;
        }
    }

    false
}

/// Normalize section header capitalization.
/// - All-caps like "FILLING" → "Filling"
/// - Mixed case like "For the Steak Fajita Marinade" → kept as-is
/// - Lowercase "for the sauce" → "For the Sauce"
fn normalize_section_name(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        return name.to_string();
    }

    // Always apply title case for consistent normalization.
    // This handles all-caps headers, lowercase headers, and mixed-case headers
    // like "For the sauce" -> "For the Sauce"
    title_case(name)
}

/// Convert a string to title case.
/// Capitalizes first letter of each word, lowercases the rest.
/// Special handling for small words (the, and, or, of, for, a, an, to, in) - kept lowercase except at start.
fn title_case(s: &str) -> String {
    const SMALL_WORDS: &[&str] = &[
        "the", "and", "or", "of", "for", "a", "an", "to", "in", "with",
    ];

    let mut result = String::with_capacity(s.len());
    let mut is_first_word = true;
    let mut current_word = String::new();

    for c in s.chars() {
        if c.is_whitespace() || c == ',' || c == '(' || c == ')' {
            // End of word - flush current word
            if !current_word.is_empty() {
                let word_lower = current_word.to_lowercase();
                if !is_first_word && SMALL_WORDS.contains(&word_lower.as_str()) {
                    result.push_str(&word_lower);
                } else {
                    // Capitalize first letter, lowercase rest
                    let mut chars = current_word.chars();
                    if let Some(first) = chars.next() {
                        result.extend(first.to_uppercase());
                        for ch in chars {
                            result.extend(ch.to_lowercase());
                        }
                    }
                }
                current_word.clear();
                is_first_word = false;
            }
            result.push(c);
        } else {
            current_word.push(c);
        }
    }

    // Flush final word
    if !current_word.is_empty() {
        let word_lower = current_word.to_lowercase();
        if !is_first_word && SMALL_WORDS.contains(&word_lower.as_str()) {
            result.push_str(&word_lower);
        } else {
            let mut chars = current_word.chars();
            if let Some(first) = chars.next() {
                result.extend(first.to_uppercase());
                for ch in chars {
                    result.extend(ch.to_lowercase());
                }
            }
        }
    }

    result
}

/// Detect if a line is a section header (e.g., "For the sauce:", "FILLING:", "Topping Ingredients:").
/// Also detects ALL CAPS lines without colons (e.g., "DOUGH", "FILLING", "BERRY SAUCE").
/// Returns Some(normalized_section_name) if it's a header, None if it's a regular ingredient.
/// Section names are normalized: "FILLING" → "Filling", "for the sauce" → "For the Sauce".
pub fn detect_section_header(raw: &str) -> Option<String> {
    let trimmed = raw.trim();

    // Check for ALL CAPS without colon first (e.g., "DOUGH", "FILLING", "BERRY SAUCE")
    // Pattern: Line is entirely uppercase letters/spaces, no digits, reasonable length
    if trimmed.len() <= 40
        && !trimmed.contains(':')
        && !trimmed.chars().any(|c| c.is_ascii_digit())
        && trimmed
            .chars()
            .filter(|c| c.is_alphabetic())
            .all(|c| c.is_uppercase())
        && trimmed.chars().any(|c| c.is_alphabetic())
    {
        return Some(normalize_section_name(trimmed));
    }

    // Must end with colon - strip it to get the section name
    let name = trimmed.strip_suffix(':')?.trim();

    // Must not be empty
    if name.is_empty() {
        return None;
    }

    // Try parsing it - if we get an amount, it's likely an ingredient, not a header
    let parsed = parse_ingredient(raw);
    if !parsed.measurements.is_empty()
        && parsed.measurements[0].amount.is_some()
        && parsed.measurements[0].unit.is_some()
    {
        // Has both amount and unit - probably an ingredient
        return None;
    }

    // Check if it matches common header patterns
    let name_lower = name.to_lowercase();

    // Pattern 1: Ends with "Ingredients" (e.g., "Topping Ingredients", "Crust Ingredients")
    if name_lower.ends_with("ingredients") || name_lower.ends_with("ingredient") {
        return Some(normalize_section_name(name));
    }

    // Pattern 2: "For the X" or "For X" patterns
    if name_lower.starts_with("for the ") || name_lower.starts_with("for ") {
        return Some(normalize_section_name(name));
    }

    // Pattern 3: All-caps short names (FILLING, DRIZZLE, TOPPING, SAUCE, etc.)
    // Must be reasonably short and mostly uppercase letters/spaces
    if name.len() <= 40
        && name
            .chars()
            .filter(|c| c.is_alphabetic())
            .all(|c| c.is_uppercase())
        && name.chars().any(|c| c.is_alphabetic())
    {
        return Some(normalize_section_name(name));
    }

    // Pattern 4: Mixed-case headers containing common section keywords
    // Must be short (typical section names are brief) and contain no digits
    if name.len() <= 50 && !name.chars().any(|c| c.is_ascii_digit()) {
        const SECTION_KEYWORDS: &[&str] = &[
            "topping", "filling", "frosting", "icing", "glaze", "sauce", "marinade", "dressing",
            "crust", "batter", "drizzle", "garnish", "assembly", "serving", "optional", "coating",
            "base", "cream", "streusel", "crumble",
        ];
        if SECTION_KEYWORDS.iter().any(|kw| name_lower.contains(kw)) {
            return Some(normalize_section_name(name));
        }
    }

    // Pattern 5: Single-word headers ending with colon (e.g., "Dough:", "Brine:", "Chicken:")
    // Must be a single word (no spaces), reasonable length, no digits
    if !name.contains(' ') && name.len() <= 20 && !name.chars().any(|c| c.is_ascii_digit()) {
        return Some(normalize_section_name(name));
    }

    None
}

/// Parse multiple ingredient lines (separated by newlines).
/// Detects section headers (lines ending with colon, no measurements) and
/// applies the section name to subsequent ingredients.
/// Skips lines that should be ignored (scraper artifacts like "Gather Your Ingredients").
pub fn parse_ingredients(blob: &str) -> Vec<ParsedIngredient> {
    let mut current_section: Option<String> = None;
    let mut results = Vec::new();

    for line in blob.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = strip_leading_list_marker(trimmed);
        let trimmed = normalized.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Skip lines that should be ignored (scraper artifacts)
        if should_ignore_line(trimmed) {
            continue;
        }

        // Check if this line is a section header
        if let Some(section_name) = detect_section_header(trimmed) {
            current_section = Some(section_name);
            continue; // Don't emit the header as an ingredient
        }

        // Parse the ingredient and apply current section
        let mut ingredient = parse_ingredient(trimmed);
        ingredient.section = current_section.clone();
        results.push(ingredient);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_section_header_for_the_pattern() {
        // "For the X" patterns should be normalized
        assert_eq!(
            detect_section_header("For the sauce:"),
            Some("For the Sauce".to_string())
        );
        assert_eq!(
            detect_section_header("For the dough:"),
            Some("For the Dough".to_string())
        );
        assert_eq!(
            detect_section_header("For serving:"),
            Some("For Serving".to_string())
        );
        // Already title case should be preserved
        assert_eq!(
            detect_section_header("For the Steak Fajita Marinade:"),
            Some("For the Steak Fajita Marinade".to_string())
        );
    }

    #[test]
    fn test_detect_section_header_all_caps() {
        // All-caps headers WITH colon should be normalized to title case
        assert_eq!(
            detect_section_header("FILLING:"),
            Some("Filling".to_string())
        );
        assert_eq!(
            detect_section_header("DRIZZLE:"),
            Some("Drizzle".to_string())
        );
        assert_eq!(
            detect_section_header("TOPPING:"),
            Some("Topping".to_string())
        );
        assert_eq!(
            detect_section_header("TOPPINGS, OPTIONAL:"),
            Some("Toppings, Optional".to_string())
        );
        assert_eq!(
            detect_section_header("FOR THE SAUCE:"),
            Some("For the Sauce".to_string())
        );
    }

    #[test]
    fn test_detect_section_header_all_caps_no_colon() {
        // All-caps headers WITHOUT colon should also be detected
        assert_eq!(
            detect_section_header("FILLING"),
            Some("Filling".to_string())
        );
        assert_eq!(detect_section_header("DOUGH"), Some("Dough".to_string()));
        assert_eq!(
            detect_section_header("ASSEMBLY"),
            Some("Assembly".to_string())
        );
        assert_eq!(
            detect_section_header("BERRY SAUCE"),
            Some("Berry Sauce".to_string())
        );
        assert_eq!(
            detect_section_header("WHIPPED COTTAGE CHEESE"),
            Some("Whipped Cottage Cheese".to_string())
        );
        assert_eq!(
            detect_section_header("TOASTS AND ASSEMBLY"),
            Some("Toasts and Assembly".to_string())
        );
    }

    #[test]
    fn test_detect_section_header_ingredients_suffix() {
        // Lines ending with "Ingredients" should be detected
        assert_eq!(
            detect_section_header("Topping Ingredients:"),
            Some("Topping Ingredients".to_string())
        );
        assert_eq!(
            detect_section_header("Crust Ingredients:"),
            Some("Crust Ingredients".to_string())
        );
        assert_eq!(
            detect_section_header("Optional Frosting Ingredients:"),
            Some("Optional Frosting Ingredients".to_string())
        );
    }

    #[test]
    fn test_detect_section_header_mixed_case_keywords() {
        // Mixed-case headers with section keywords should be detected
        assert_eq!(
            detect_section_header("Toppings:"),
            Some("Toppings".to_string())
        );
        assert_eq!(
            detect_section_header("optional toppings:"),
            Some("Optional Toppings".to_string())
        );
        assert_eq!(
            detect_section_header("Cream cheese filling:"),
            Some("Cream Cheese Filling".to_string())
        );
        assert_eq!(
            detect_section_header("Chocolate Glaze:"),
            Some("Chocolate Glaze".to_string())
        );
        assert_eq!(
            detect_section_header("For serving:"),
            Some("For Serving".to_string())
        );
    }

    #[test]
    fn test_detect_section_header_single_word() {
        // Single-word headers ending with colon should be detected
        assert_eq!(detect_section_header("Dough:"), Some("Dough".to_string()));
        assert_eq!(detect_section_header("Brine:"), Some("Brine".to_string()));
        assert_eq!(
            detect_section_header("Chicken:"),
            Some("Chicken".to_string())
        );
        assert_eq!(detect_section_header("Eggs:"), Some("Eggs".to_string()));
        assert_eq!(
            detect_section_header("Caramel:"),
            Some("Caramel".to_string())
        );
        assert_eq!(
            detect_section_header("Meatballs:"),
            Some("Meatballs".to_string())
        );
    }

    #[test]
    fn test_detect_section_header_not_header() {
        // Regular ingredients should return None
        assert_eq!(detect_section_header("1 cup flour"), None);
        assert_eq!(detect_section_header("2 tablespoons oil"), None);
        // Ingredient with colon in note should not be detected as header
        assert_eq!(detect_section_header("butter: softened"), None);
    }

    #[test]
    fn test_should_ignore_line() {
        // Scraper artifacts should be ignored
        assert!(should_ignore_line("Gather Your Ingredients"));
        assert!(should_ignore_line("gather your ingredients"));
        assert!(should_ignore_line("GATHER YOUR INGREDIENTS"));
        assert!(should_ignore_line("Special equipment: Spice grinder"));
        assert!(should_ignore_line("Equipment: Stand mixer"));
        assert!(should_ignore_line("Notes: See recipe headnotes"));

        // Regular ingredients should not be ignored
        assert!(!should_ignore_line("1 cup flour"));
        assert!(!should_ignore_line("salt to taste"));
        assert!(!should_ignore_line("For the sauce:"));
    }

    #[test]
    fn test_parse_ingredients_with_sections() {
        let blob = "For the sauce:\n1 cup tomatoes\n2 tbsp oil\nFor the pasta:\n1 lb spaghetti";
        let result = parse_ingredients(blob);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].section, Some("For the Sauce".to_string()));
        assert_eq!(result[0].item, "tomatoes");
        assert_eq!(result[1].section, Some("For the Sauce".to_string()));
        assert_eq!(result[1].item, "oil");
        assert_eq!(result[2].section, Some("For the Pasta".to_string()));
        assert_eq!(result[2].item, "spaghetti");
    }

    #[test]
    fn test_parse_ingredients_no_sections() {
        let blob = "1 cup flour\n2 eggs\n1 tsp salt";
        let result = parse_ingredients(blob);

        assert_eq!(result.len(), 3);
        assert!(result[0].section.is_none());
        assert!(result[1].section.is_none());
        assert!(result[2].section.is_none());
    }

    #[test]
    fn test_parse_ingredients_section_headers_removed() {
        let blob = "FILLING:\n1 cup ricotta\nTOPPING:\n1/2 cup cheese";
        let result = parse_ingredients(blob);

        // Section headers should not appear as ingredients
        // Section names should be normalized to title case
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].item, "ricotta");
        assert_eq!(result[0].section, Some("Filling".to_string()));
        assert_eq!(result[1].item, "cheese");
        assert_eq!(result[1].section, Some("Topping".to_string()));
    }

    #[test]
    fn test_parse_ingredients_all_caps_no_colon_sections() {
        // ALL CAPS without colon should also be detected as section headers
        let blob = "DOUGH\n2 cups flour\n1 tsp yeast\nFILLING\n1 cup onions";
        let result = parse_ingredients(blob);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].item, "flour");
        assert_eq!(result[0].section, Some("Dough".to_string()));
        assert_eq!(result[1].item, "yeast");
        assert_eq!(result[1].section, Some("Dough".to_string()));
        assert_eq!(result[2].item, "onions");
        assert_eq!(result[2].section, Some("Filling".to_string()));
    }

    #[test]
    fn test_parse_ingredients_ignores_scraper_artifacts() {
        let blob = "Gather Your Ingredients\n1 cup flour\nSpecial equipment: Stand mixer\n2 eggs";
        let result = parse_ingredients(blob);

        // Scraper artifacts should be filtered out
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].item, "flour");
        assert_eq!(result[1].item, "eggs");
    }

    #[test]
    fn test_title_case() {
        assert_eq!(title_case("FILLING"), "Filling");
        assert_eq!(title_case("FOR THE SAUCE"), "For the Sauce");
        assert_eq!(title_case("TOPPINGS, OPTIONAL"), "Toppings, Optional");
        assert_eq!(title_case("for the dough"), "For the Dough");
        assert_eq!(
            title_case("FOR THE STEAK FAJITA MARINADE"),
            "For the Steak Fajita Marinade"
        );
    }
}
