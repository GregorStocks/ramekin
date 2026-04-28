//! Ingredient parsing module.
//!
//! Parses raw ingredient strings (e.g., "2 cups flour, sifted") into structured data.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::metric_weights::parse_amount;

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeferredParentheticalNote {
    segment: String,
    follows_comma: bool,
}

impl ParsedIngredient {
    /// Normalize fraction amounts to decimal form across all measurements.
    /// Should be called after metric/volume enrichment to avoid rounding errors.
    pub fn normalize_amounts(mut self) -> Self {
        for m in &mut self.measurements {
            if let Some(ref amount) = m.amount {
                m.amount = Some(normalize_fraction_to_decimal(amount));
            }
        }
        self
    }
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
    "reserved",
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
/// - Unicode fraction slash (⁄) → ASCII slash
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

            // Fraction slash → ASCII slash
            '⁄' => result.push('/'),

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

/// Format a decimal amount, stripping trailing zeros.
/// "0.50" -> "0.5", "1.00" -> "1", "2.50" -> "2.5"
fn format_decimal_amount(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded == rounded.floor() {
        format!("{}", rounded as i64)
    } else {
        let s = format!("{:.2}", rounded);
        s.trim_end_matches('0').to_string()
    }
}

/// Normalize a single amount value to clean decimal form.
/// Fractions: "1/2" -> "0.5", "3/4" -> "0.75", "1 1/2" -> "1.5"
/// Ugly decimals: "0.33333334326744" -> "0.33"
/// Clean values pass through: "2" -> "2", "0.5" -> "0.5"
fn normalize_single_amount(s: &str) -> String {
    match parse_amount(s) {
        Some(value) => {
            let formatted = format_decimal_amount(value);
            // Only use the formatted version if it's different (i.e., we actually simplified)
            // or if the original contained a fraction
            if s.contains('/') || formatted.len() < s.len() {
                formatted
            } else {
                s.to_string()
            }
        }
        None => s.to_string(),
    }
}

/// Find the index of a range hyphen in an amount string.
/// Distinguishes real ranges ("6-8") from mixed-number notation ("1-1/2").
fn find_range_hyphen_in_amount(s: &str) -> Option<usize> {
    for (i, c) in s.char_indices() {
        if c == '-' && i > 0 {
            let before = s.get(..i)?.chars().last()?;
            let after_part = s.get(i + 1..)?;
            let after = after_part.chars().next()?;
            if before.is_ascii_digit() && after.is_ascii_digit() {
                let first_token = after_part.split_whitespace().next().unwrap_or("");
                if !first_token.contains('/') {
                    return Some(i);
                }
            }
        }
    }
    None
}

/// Normalize amounts to clean decimal form.
/// Fractions: "1/2" -> "0.5", "3/4" -> "0.75", "1 1/2" -> "1.5"
/// Repeating fractions round to 2 decimal places: "1/3" -> "0.33"
/// Ugly decimals: "0.33333334326744" -> "0.33"
/// Clean values pass through unchanged.
fn normalize_fraction_to_decimal(amount: &str) -> String {
    let amount = amount.trim();

    // Handle ranges with " to "
    if let Some((low, high)) = amount.split_once(" to ") {
        let low_converted = normalize_single_amount(low.trim());
        let high_converted = normalize_single_amount(high.trim());
        return format!("{} to {}", low_converted, high_converted);
    }

    // Handle ranges with " or "
    if let Some((low, high)) = amount.split_once(" or ") {
        let low_converted = normalize_single_amount(low.trim());
        let high_converted = normalize_single_amount(high.trim());
        return format!("{} or {}", low_converted, high_converted);
    }

    // Handle hyphenated ranges: "1 1/2-3" or "2 1/2-4 1/2"
    if let Some(hyphen_idx) = find_range_hyphen_in_amount(amount) {
        if let (Some(low), Some(high)) = (amount.get(..hyphen_idx), amount.get(hyphen_idx + 1..)) {
            let low_converted = normalize_single_amount(low.trim());
            let high_converted = normalize_single_amount(high.trim());
            return format!("{}-{}", low_converted, high_converted);
        }
    }

    // Single value
    normalize_single_amount(amount)
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

    // Strip "More " prefix (e.g., "More parsley" → "parsley")
    // These are typically garnish additions in ingredient lists
    let remaining_lower = remaining.to_lowercase();
    if remaining_lower.starts_with("more ") {
        remaining = remaining.get(5..).unwrap_or("").trim().to_string();
    }

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

    // Issue 5: Normalize double-wrapped parentheticals to single
    // e.g., "((about 4 cloves))" -> "(about 4 cloves)"
    remaining = unwrap_redundant_parentheses(&remaining);

    // Strip placeholder parentheticals: TK ("to come") and TODO markers
    // e.g., "1/2 cup (TK g) panko bread crumbs" -> "1/2 cup panko bread crumbs"
    // e.g., "(TODO) chicken stock" -> "chicken stock"
    loop {
        let lower = remaining.to_lowercase();
        let Some(start) = lower.find("(tk").or_else(|| lower.find("(todo")) else {
            break;
        };
        let after = match remaining.get(start..) {
            Some(s) => s,
            None => break,
        };
        let Some(end_offset) = after.find(')') else {
            break;
        };
        let before = remaining.get(..start).unwrap_or("").trim_end();
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
    let mut deferred_parenthetical_notes = Vec::new();
    while let Some(start) = remaining.find('(') {
        let Some(close_idx) = find_matching_closing_paren(&remaining, start) else {
            break;
        };
        let paren_content = match remaining.get(start + 1..close_idx) {
            Some(s) => s,
            None => break,
        };

        let raw_before_parenthetical = remaining.get(..start).unwrap_or("");
        let follows_comma = raw_before_parenthetical.trim_end().ends_with(',')
            || raw_before_parenthetical.contains(',');
        let before_parenthetical = raw_before_parenthetical
            .trim_end()
            .trim_end_matches(',')
            .trim_end();
        let after_parenthetical = remaining.get(close_idx + 1..).unwrap_or("").trim_start();

        if let Some((item_segment, note_segment)) = split_parenthetical_item_identity(paren_content)
        {
            let outside_text = join_segments(before_parenthetical, after_parenthetical);
            if outside_lacks_item_identity(&outside_text) {
                if let Some(note_segment) = note_segment {
                    push_deferred_parenthetical_note(
                        &mut deferred_parenthetical_notes,
                        &note_segment,
                        follows_comma,
                    );
                }
                remaining = join_segments(
                    &join_segments(before_parenthetical, &item_segment),
                    after_parenthetical,
                );
                continue;
            }
        }

        // First check if this is a prep note (like "softened", "chopped", etc.)
        // Skip if content starts with a digit AND has nested parens - that's a measurement, not prep note
        // e.g., "(15.5 oz (liquid reserved))" should parse as measurement, not prep note
        // but "(quartered (approx. 15 mushrooms))" is a valid prep note
        let has_nested_parens = paren_content.contains('(') || paren_content.contains(')');
        let starts_with_digit = paren_content
            .trim()
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit());
        let skip_prep_note = has_nested_parens && starts_with_digit;
        if is_prep_note(paren_content) && !skip_prep_note {
            // Strip leading comma (e.g., from raw like "tomato (, sliced)")
            let trimmed_content = paren_content
                .trim()
                .trim_start_matches(',')
                .trim()
                .to_string();
            push_deferred_parenthetical_note(
                &mut deferred_parenthetical_notes,
                &trimmed_content,
                follows_comma,
            );
            // Remove the parenthetical from remaining
            // Also strip trailing comma before the parenthetical (e.g., "onion, (diced)")
            remaining = join_segments(before_parenthetical, after_parenthetical);
            continue;
        }

        // Try to parse the parenthetical content as one or more measurements
        // Split by semicolons or commas to handle "8 ounces; 227 g each"
        let parsed_measurements = parse_parenthetical_measurements(paren_content);

        if !parsed_measurements.is_empty() {
            alt_measurements.extend(parsed_measurements);
            // Remove the parenthetical from remaining, preserving space
            // Also strip trailing comma before the parenthetical
            remaining = join_segments(before_parenthetical, after_parenthetical);
        } else {
            // Preserve any non-measurement parenthetical as ingredient-relevant note
            // instead of dropping it on the floor.
            let trimmed_content = paren_content
                .trim()
                .trim_start_matches(',')
                .trim()
                .to_string();
            push_deferred_parenthetical_note(
                &mut deferred_parenthetical_notes,
                &trimmed_content,
                follows_comma,
            );
            remaining = join_segments(before_parenthetical, after_parenthetical);
        }
    }

    // Step 2: Handle "plus" patterns
    // ", plus " -> always extract as note (e.g., "flour, plus more for dusting")
    // " plus " -> check if followed by valid measurement (amount+unit)
    //   - If valid measurement: handled later as compound amount (e.g., "1 cup plus 2 tbsp flour")
    //   - If no valid measurement: extract as note (e.g., "1 egg plus 1 yolk")
    if let Some(plus_idx) = remaining.to_lowercase().find(", plus ") {
        // With comma: always extract as note
        if let Some(plus_part) = remaining.get(plus_idx + 2..) {
            if note.is_none() {
                note = Some(plus_part.trim().to_string());
            }
        }
        remaining = remaining.get(..plus_idx).unwrap_or("").trim().to_string();
    } else if let Some(plus_idx) = remaining.to_lowercase().find(" plus ") {
        // Without comma: check if followed by valid measurement
        let after_plus = remaining.get(plus_idx + 6..).unwrap_or("").trim();
        if !after_plus.is_empty() {
            let (test_amount, after_test_amount) = extract_amount(after_plus);
            let (test_unit, _) = extract_unit(&after_test_amount);

            // Only if we DON'T have both amount+unit, treat as note (fallback)
            // Cases with valid measurement are handled later as compound amounts
            if test_amount.is_none() || test_unit.is_none() {
                if let Some(plus_part) = remaining.get(plus_idx + 1..) {
                    if note.is_none() {
                        note = Some(plus_part.trim().to_string());
                    }
                }
                remaining = remaining.get(..plus_idx).unwrap_or("").trim().to_string();
            }
        }
    }

    // Step 3: Strip measurement modifiers before amount, preserve for unit
    // Handles "scant 1 teaspoon" - modifier goes on the unit as "scant teaspoon"
    let (pre_amount_modifier, after_modifier) = strip_measurement_modifier(&remaining);
    remaining = after_modifier;

    let (mut primary_amount, after_amount) = extract_amount(&remaining);
    remaining = after_amount;

    // Step 4: Strip measurement modifiers before unit, combine with any pre-amount modifier
    // Handles "2 heaping tablespoons" - modifier goes on the unit as "heaping tablespoons"
    let (pre_unit_modifier, after_modifier) = strip_measurement_modifier(&remaining);
    remaining = after_modifier;

    let multiplier_unit = primary_amount
        .as_ref()
        .and_then(|_| try_extract_multiplier_unit(&remaining));

    let (mut base_unit, mut after_unit) = if let Some((unit, after_multiplier)) = multiplier_unit {
        (Some(unit), after_multiplier)
    } else {
        extract_unit(&remaining)
    };

    // Step 4a: Handle "N unit container" compound units (e.g., "14 ounce can")
    // If no unit was found, check if remaining starts with a compound unit pattern
    if base_unit.is_none() {
        if let Some((compound_unit, after_compound)) = try_extract_compound_unit(&remaining) {
            base_unit = Some(compound_unit);
            after_unit = after_compound;
        } else if let Some(amount_str) = primary_amount.as_deref() {
            if let Some(((replacement_amount, recovered_unit), after_recovered)) =
                try_extract_hyphenated_unit_tail(amount_str, &remaining)
            {
                primary_amount = Some(replacement_amount);
                base_unit = Some(recovered_unit);
                after_unit = after_recovered;
            }
        }
    }

    remaining = after_unit;

    // Step 4.0.5: Strip orphaned "of" + article after amount extraction with no unit.
    // Handles "Half of a lemon" → after amount "1/2" extracted, remaining is
    // "of a lemon". With no unit to absorb the "of", strip it here.
    if primary_amount.is_some() && base_unit.is_none() {
        let remaining_trimmed = remaining.trim_start();
        let remaining_lower = remaining_trimmed.to_lowercase();
        if remaining_lower.starts_with("of ") {
            let after_of = remaining_trimmed.get(3..).unwrap_or("").trim_start();
            let after_of_lower = after_of.to_lowercase();
            remaining = if after_of_lower.starts_with("a ") {
                after_of.get(2..).unwrap_or("").trim_start().to_string()
            } else if after_of_lower.starts_with("an ") {
                after_of.get(3..).unwrap_or("").trim_start().to_string()
            } else if after_of_lower.starts_with("the ") {
                after_of.get(4..).unwrap_or("").trim_start().to_string()
            } else {
                after_of.to_string()
            };
        }
    }

    // Step 4.1: Move leading "each" from remaining text onto the unit
    // Handles "1/2 tsp each salt and pepper" -> unit becomes "tsp each", item becomes "salt and pepper"
    // "each" here means "this measurement applies to each of the following items"
    // This is consistent with how parenthetical "each" is handled (e.g., "(8 oz each)" -> unit: "oz each")
    {
        let remaining_trimmed = remaining.trim_start();
        if remaining_trimmed.to_lowercase().starts_with("each ") {
            remaining = remaining_trimmed.get(5..).unwrap_or("").to_string();
            base_unit = Some(match base_unit {
                Some(u) => format!("{} each", u),
                None => "each".to_string(),
            });
        }
    }

    // Combine modifiers with unit: prefer pre-unit modifier, fall back to pre-amount modifier
    let modifier = pre_unit_modifier.or(pre_amount_modifier);
    let mut primary_unit = match (modifier, base_unit) {
        (Some(m), Some(u)) => Some(format!("{} {}", m, u)),
        (Some(m), None) => Some(m), // modifier without unit (rare but possible)
        (None, u) => u,
    };

    // Step 4.4a: Handle repeated-unit ranges like "1/2 cup to 1 cup white rum".
    // If the second half parses as another measurement with the same unit, fold it
    // into the primary amount and keep the shared unit once.
    {
        let remaining_trimmed = remaining.trim_start();
        if let Some(after_to) = remaining_trimmed.strip_prefix("to ") {
            let after_to = after_to.trim_start();
            if let Some((upper_amount, upper_unit, after_to_unit)) =
                parse_range_continuation_measurement(after_to)
            {
                match (
                    primary_amount.as_ref(),
                    primary_unit.as_ref(),
                    upper_unit.as_ref(),
                ) {
                    (Some(amount), Some(unit), Some(upper_unit))
                        if units_share_base(unit, upper_unit) =>
                    {
                        primary_amount = Some(format!("{} to {}", amount, upper_amount));
                        remaining = after_to_unit;
                    }
                    (Some(amount), Some(unit), Some(upper_unit)) => {
                        primary_amount = Some(format!(
                            "{} {} to {} {}",
                            amount, unit, upper_amount, upper_unit
                        ));
                        primary_unit = None;
                        remaining = after_to_unit;
                    }
                    (Some(amount), None, Some(upper_unit)) => {
                        primary_amount = Some(format!("{} to {}", amount, upper_amount));
                        primary_unit = Some(upper_unit.clone());
                        remaining = after_to_unit;
                    }
                    _ => {}
                }
            }
        }
    }

    // Step 4.4b: Handle "plus [amount] [unit]" compound quantities
    // e.g., "1/2 cup plus 2 tablespoons flour" -> amount="1/2 cup plus 2 tablespoons", unit=null
    // This keeps the compound quantity together as a single amount rather than splitting into note
    {
        let remaining_trimmed = remaining.trim_start();
        let remaining_lower = remaining_trimmed.to_lowercase();
        if remaining_lower.starts_with("plus ") {
            let after_plus = remaining_trimmed.get(5..).unwrap_or("").trim_start();

            // Try to parse a measurement from what follows "plus"
            let (plus_amount, after_plus_amount) = extract_amount(after_plus);
            let (plus_unit, after_plus_unit) = extract_unit(&after_plus_amount);

            // Only combine if we got BOTH amount AND unit after "plus"
            if let (Some(p_amt), Some(p_unit)) = (plus_amount, plus_unit) {
                // Combine into a single compound amount: "1/2 cup plus 2 tablespoons"
                if let (Some(amt), Some(unit)) = (&primary_amount, &primary_unit) {
                    primary_amount = Some(format!("{} {} plus {} {}", amt, unit, p_amt, p_unit));
                    primary_unit = None; // Unit is now embedded in the compound amount
                    remaining = after_plus_unit;
                }
            }
        }
    }

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
        let Some(after_slash) = remaining_trimmed.strip_prefix('/') else {
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
    // But don't extract if it would leave only prep words as the item
    if note.is_none() {
        if let Some(comma_idx) = remaining.rfind(',') {
            if let Some(potential_note) = remaining.get(comma_idx + 1..) {
                let potential_note = potential_note.trim();
                let potential_item = remaining.get(..comma_idx).unwrap_or("").trim();
                // Check if it looks like a prep note AND extracting it wouldn't
                // leave only prep words as the item
                if is_trailing_prep_note(potential_note) && !is_only_prep_words(potential_item) {
                    note = Some(potential_note.to_string());
                    remaining = potential_item.to_string();
                }
            }
        }
    }

    // Step 5.5: Handle " or " alternatives in the MIDDLE of remaining text
    // e.g., remaining = "dried Italian seasoning or a combination of 1/4 teaspoon dried oregano..."
    // The text before " or " becomes the item, and "or ..." goes into the note
    // Only apply if:
    // - Note isn't already set
    // - The text after " or " contains a measurement somewhere (indicating an alternative preparation)
    // This avoids breaking compound noun phrases like "chicken or vegetable stock"
    if note.is_none() {
        if let Some(or_idx) = remaining.to_lowercase().find(" or ") {
            let before_or = remaining.get(..or_idx).unwrap_or("").trim();
            let after_or = remaining.get(or_idx + 4..).unwrap_or("").trim(); // Skip " or "

            // Check if after_or contains a measurement pattern (number + unit)
            // This catches cases like "a combination of 1/4 teaspoon..."
            // We require BOTH a number AND a unit to be present to avoid false positives
            // Only check content before the first comma (to avoid "orange or lemon zest, 1 tsp...")
            // Exclude content in parentheses from the check (e.g., "(I used 5 cups...)")
            let after_or_lower = after_or.to_lowercase();
            // Get content before first comma, then remove parentheticals
            let before_comma = after_or_lower.split(',').next().unwrap_or(&after_or_lower);
            let without_parens: String = {
                let mut result = String::new();
                let mut depth: i32 = 0;
                for c in before_comma.chars() {
                    match c {
                        '(' | '[' => depth += 1,
                        ')' | ']' => depth = depth.saturating_sub(1),
                        _ if depth == 0 => result.push(c),
                        _ => {}
                    }
                }
                result
            };
            let unit_patterns = [
                "teaspoon",
                "tsp",
                "tablespoon",
                "tbsp",
                "cup",
                "cups",
                "ounce",
                "ounces",
                "oz",
                "pound",
                "pounds",
                "lb",
                "lbs",
                "gram",
                "grams",
                "kg",
                "ml",
                "liter",
                "liters",
                "pinch",
                "dash",
                "handful",
                "bunch",
            ];
            let has_unit = without_parens.split_whitespace().any(|word| {
                unit_patterns
                    .iter()
                    .any(|p| word == *p || word.starts_with(p))
            });
            let has_number = without_parens.chars().any(|c| c.is_ascii_digit());
            let contains_measurement = has_unit && has_number;

            if !before_or.is_empty() && !after_or.is_empty() && contains_measurement {
                note = Some(format!("or {}", after_or));
                remaining = before_or.to_string();
            }
        }
    }

    // Step 5.7: Handle comma-separated conditional alternatives in remaining text
    // e.g., remaining = "vegetable broth, 1 1/2 cups vegetable broth (for cooked chickpeas)"
    // Pattern: [item], [amount] [unit] [same item] ([condition])
    // The repeated item name distinguishes this from other comma uses.
    if let Some(comma_idx) = remaining.find(',') {
        let before_comma = remaining.get(..comma_idx).unwrap_or("").trim();
        let after_comma = remaining.get(comma_idx + 1..).unwrap_or("").trim();

        if !before_comma.is_empty() && !after_comma.is_empty() {
            let (alt_amount, after_alt_amount) = extract_amount(after_comma);
            let (alt_unit, after_alt_unit) = extract_unit(&after_alt_amount);

            if let (Some(ref amt), Some(ref unit)) = (&alt_amount, &alt_unit) {
                let after_alt_unit_trimmed = after_alt_unit.trim();
                let before_comma_lower = before_comma.to_lowercase();
                let after_unit_lower = after_alt_unit_trimmed.to_lowercase();

                if after_unit_lower.starts_with(&before_comma_lower) {
                    // Item name repeats after the alternative measurement — this is a conditional
                    let conditional_part = after_alt_unit_trimmed
                        .get(before_comma.len()..)
                        .unwrap_or("")
                        .trim()
                        .trim_start_matches('(')
                        .trim_end_matches(')')
                        .trim();

                    let alternate_condition = if conditional_part.is_empty() {
                        take_last_comma_parenthetical_note(&mut deferred_parenthetical_notes)
                    } else {
                        Some(conditional_part.to_string())
                    };

                    let fragment = if let Some(alternate_condition) = alternate_condition {
                        format!(
                            "or {} {} {}",
                            amt,
                            normalize_unit(unit),
                            alternate_condition
                        )
                    } else {
                        format!("or {} {}", amt, normalize_unit(unit))
                    };

                    remaining = before_comma.to_string();
                    note = match note {
                        Some(existing) => Some(format!("{}; {}", existing, fragment)),
                        None => Some(fragment),
                    };
                }
            }
        }
    }

    prepend_deferred_parenthetical_notes(&mut note, &deferred_parenthetical_notes);

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
    // Strip trailing footnote markers (*, **, ***) from recipe sites
    // Strip trailing commas (e.g., "pork tenderloins,")
    // Strip trailing semicolons (e.g., "cheese, grated;" when semicolon was separator before note)
    // Normalize " ," to "," (space before comma from parenthetical extraction)
    let item = remaining
        .trim()
        .trim_start_matches(',')
        .trim()
        .trim_end_matches(" )")
        .trim()
        .trim_end_matches(',')
        .trim_end_matches(';')
        .trim_end_matches('*')
        .trim()
        .replace(" ,", ",")
        .to_string();

    // Strip trailing footnote markers from note (e.g., "at room temperature**" → "at room temperature")
    if let Some(ref n) = note {
        let trimmed = n.trim_end_matches('*').trim_end().to_string();
        note = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
    }

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

    // If item is only prep words, the parse failed - return raw as item
    // This handles cases like "⅓ cup toasted (chopped pistachios)" where
    // parenthetical extraction leaves only a prep word as the item
    if is_only_prep_words(&item) && !item.is_empty() {
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

fn try_parse_measurement_with_options(
    s: &str,
    allow_custom_count_units: bool,
) -> Option<Measurement> {
    let s = s.trim();
    let (mut amount, after_amount) = extract_amount(s);
    let (mut unit, mut remaining) = extract_unit(&after_amount);

    if unit.is_none() {
        if let Some(amount_str) = amount.as_deref() {
            if let Some(((replacement_amount, recovered_unit), after_recovered)) =
                try_extract_hyphenated_unit_tail(amount_str, &after_amount)
            {
                amount = Some(replacement_amount);
                unit = Some(recovered_unit);
                remaining = after_recovered;
            }
        }
    }

    if allow_custom_count_units
        && unit.is_none()
        && amount.is_some()
        && looks_like_parenthetical_count_unit(remaining.trim())
    {
        unit = Some(remaining.trim().to_string());
        remaining = String::new();
    }

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
    let normalized = normalize_parenthetical_measurement_separators(content)
        .replace(" or ", ";")
        .replace(" Or ", ";")
        .replace(" OR ", ";");

    // Split by semicolons or commas (common separators in recipe measurements)
    for part in normalized.split([';', ',']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some(mut m) = try_parse_parenthetical_measurement(part) {
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
        } else {
            return Vec::new();
        }
    }

    results
}

fn normalize_parenthetical_measurement_separators(content: &str) -> String {
    let chars: Vec<char> = content.chars().collect();
    let mut normalized = String::with_capacity(content.len() + 4);
    let mut i = 0;

    while i < chars.len() {
        if chars[i] != '/' {
            normalized.push(chars[i]);
            i += 1;
            continue;
        }

        let prev = chars[..i]
            .iter()
            .rev()
            .find(|c| !c.is_whitespace())
            .copied();
        let next = chars[i + 1..].iter().find(|c| !c.is_whitespace()).copied();

        let is_measurement_separator = prev.is_some_and(|c| c.is_ascii_alphabetic() || c == '.')
            && next.is_some_and(|c| c.is_ascii_digit());

        if is_measurement_separator {
            while normalized.ends_with(' ') {
                normalized.pop();
            }
            normalized.push(';');
            normalized.push(' ');
            i += 1;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            continue;
        }

        normalized.push('/');
        i += 1;
    }

    normalized
}

fn try_parse_parenthetical_measurement(s: &str) -> Option<Measurement> {
    let cleaned = strip_measurement_qualifiers(s);
    let measurement = try_parse_measurement_with_options(&cleaned, false)?;
    let (amount, unit) = match (measurement.amount, measurement.unit) {
        (Some(amount), Some(unit)) => (amount, unit),
        _ => return None,
    };

    let normalized_unit = normalize_unit(&unit);
    let normalized_base = normalized_unit
        .strip_suffix(" each")
        .unwrap_or(&normalized_unit);
    if normalized_base == "inch" {
        return None;
    }

    if looks_like_bare_parenthetical_size_unit(&cleaned, normalized_base) {
        return None;
    }

    Some(Measurement {
        amount: Some(amount),
        unit: Some(unit),
    })
}

fn looks_like_bare_parenthetical_size_unit(s: &str, normalized_unit: &str) -> bool {
    const BARE_PARENTHEICAL_SIZE_UNITS: &[&str] = &["oz", "lb", "inch", "fl oz"];

    if !BARE_PARENTHEICAL_SIZE_UNITS.contains(&normalized_unit) {
        return false;
    }

    let trimmed = s.trim().trim_end_matches('.').to_lowercase();
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.len() != 1 {
        return false;
    }

    let Some((amount_part, unit_part)) = words[0].split_once('-') else {
        return false;
    };
    if !is_amount_like(amount_part) {
        return false;
    }

    normalize_unit(unit_part.trim_end_matches('.')) == normalized_unit
}

fn looks_like_parenthetical_count_unit(s: &str) -> bool {
    if s.is_empty() || s.contains(['(', ')', '[', ']', ':', ';', ',']) {
        return false;
    }

    const DISALLOWED_WORDS: &[&str] = &[
        "a",
        "an",
        "and",
        "approx",
        "approximately",
        "around",
        "as",
        "at",
        "about",
        "by",
        "depending",
        "extra",
        "for",
        "from",
        "if",
        "more",
        "of",
        "or",
        "per",
        "plus",
        "see",
        "to",
        "with",
    ];
    const DESCRIPTOR_WORDS: &[&str] = &[
        "big", "jumbo", "large", "medium", "meaty", "mini", "small", "tiny",
    ];

    let words: Vec<&str> = s.split_whitespace().collect();
    if words.is_empty() {
        return false;
    }

    let mut has_noun = false;
    for word in words {
        if !word.chars().all(|c| c.is_ascii_alphabetic() || c == '-') {
            return false;
        }

        let normalized = word.to_lowercase();
        if normalized.is_empty() {
            return false;
        }
        if DISALLOWED_WORDS.contains(&normalized.as_str()) {
            return false;
        }
        if normalized.chars().any(|c| c.is_ascii_digit()) {
            return false;
        }
        if !DESCRIPTOR_WORDS.contains(&normalized.as_str()) {
            has_noun = true;
        }
    }

    has_noun
}

fn push_deferred_parenthetical_note(
    notes: &mut Vec<DeferredParentheticalNote>,
    segment: &str,
    follows_comma: bool,
) {
    let segment = segment.trim();
    if segment.is_empty() {
        return;
    }

    notes.push(DeferredParentheticalNote {
        segment: segment.to_string(),
        follows_comma,
    });
}

fn take_last_comma_parenthetical_note(
    notes: &mut Vec<DeferredParentheticalNote>,
) -> Option<String> {
    let index = notes.iter().rposition(|note| note.follows_comma)?;
    Some(notes.remove(index).segment)
}

fn prepend_deferred_parenthetical_notes(
    note: &mut Option<String>,
    deferred_notes: &[DeferredParentheticalNote],
) {
    let parenthetical_note = deferred_notes
        .iter()
        .map(|note| note.segment.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    if parenthetical_note.is_empty() {
        return;
    }

    *note = Some(match note.take() {
        Some(existing_note) if existing_note.starts_with("or ") => {
            format!("{}; {}", parenthetical_note, existing_note)
        }
        Some(existing_note) => format!("{}, {}", parenthetical_note, existing_note),
        None => parenthetical_note,
    });
}

fn join_segments(left: &str, right: &str) -> String {
    if left.is_empty() {
        right.to_string()
    } else if right.is_empty() {
        left.to_string()
    } else {
        format!("{} {}", left, right)
    }
}

fn split_parenthetical_item_identity(content: &str) -> Option<(String, Option<String>)> {
    let cleaned = content.trim().trim_start_matches(',').trim();
    if cleaned.is_empty() {
        return None;
    }

    let split_idx = cleaned
        .char_indices()
        .find_map(|(idx, ch)| (ch == ',' || ch == ';').then_some(idx));
    let (item_segment, note_segment) = match split_idx {
        Some(idx) => {
            let item_segment = cleaned.get(..idx).unwrap_or("").trim();
            let note_segment = cleaned.get(idx + 1..).unwrap_or("").trim();
            (item_segment, Some(note_segment))
        }
        None => (cleaned, None),
    };

    let identity_check_segment = item_segment
        .strip_prefix("or ")
        .or_else(|| item_segment.strip_prefix("Or "))
        .or_else(|| item_segment.strip_prefix("OR "))
        .map(str::trim)
        .unwrap_or(item_segment);

    if !looks_like_parenthetical_item_identity(identity_check_segment) {
        return None;
    }

    Some((
        item_segment.to_string(),
        note_segment
            .filter(|segment| !segment.is_empty())
            .map(str::to_string),
    ))
}

fn looks_like_parenthetical_item_identity(s: &str) -> bool {
    const GUIDANCE_PREFIXES: &[&str] = &[
        "about",
        "approx",
        "approximately",
        "around",
        "as",
        "at",
        "depending",
        "for",
        "from",
        "if",
        "less",
        "more",
        "note",
        "or",
        "per",
        "plus",
        "see",
        "to",
        "with",
    ];
    const NON_NOUN_WORDS: &[&str] = &[
        "a", "an", "and", "big", "black", "brown", "chopped", "diced", "green", "large", "mashed",
        "medium", "mini", "orange", "purple", "red", "ripe", "small", "sweet", "the", "tiny",
        "well", "white", "yellow",
    ];

    if s.contains(['(', ')']) {
        return false;
    }

    let mut words = s.split_whitespace().peekable();
    let Some(first_word) = words.peek() else {
        return false;
    };
    let first_normalized = normalize_identity_word(first_word);
    if first_normalized.is_empty() || GUIDANCE_PREFIXES.contains(&first_normalized.as_str()) {
        return false;
    }

    let mut has_noun = false;
    for word in s.split_whitespace() {
        let normalized = normalize_identity_word(word);
        if normalized.is_empty() {
            continue;
        }
        if normalized.chars().any(|c| c.is_ascii_digit()) {
            return false;
        }
        if !normalized
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c == '-')
        {
            return false;
        }
        if NON_NOUN_WORDS.contains(&normalized.as_str()) {
            continue;
        }
        if PREP_NOTES
            .iter()
            .any(|note| normalized == *note || note.starts_with(&format!("{} ", normalized)))
        {
            continue;
        }

        has_noun = true;
    }

    has_noun
}

fn normalize_identity_word(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_ascii_alphabetic() && c != '-')
        .to_lowercase()
}

fn outside_lacks_item_identity(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return true;
    }

    let (_, after_pre_amount_modifier) = strip_measurement_modifier(s);
    let (_, after_amount) = extract_amount(&after_pre_amount_modifier);
    let (_, after_pre_unit_modifier) = strip_measurement_modifier(&after_amount);
    let (_, after_unit) = extract_unit(&after_pre_unit_modifier);
    let candidate = after_unit.trim().trim_start_matches(',').trim();

    candidate.is_empty() || is_only_descriptor_or_prep_words(candidate)
}

fn is_only_descriptor_or_prep_words(s: &str) -> bool {
    const DESCRIPTOR_WORDS: &[&str] = &[
        "big", "black", "brown", "green", "jumbo", "large", "medium", "meaty", "mini", "orange",
        "purple", "red", "ripe", "small", "sweet", "tiny", "white", "yellow",
    ];

    let mut saw_word = false;
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if is_only_prep_words(part) {
            saw_word = true;
            continue;
        }

        let words: Vec<&str> = part.split_whitespace().collect();
        if words.is_empty() {
            continue;
        }

        let all_descriptors = words.iter().all(|word| {
            let normalized = word
                .trim_matches(|c: char| !c.is_ascii_alphabetic())
                .to_lowercase();
            DESCRIPTOR_WORDS.contains(&normalized.as_str())
        });
        if !all_descriptors {
            return false;
        }
        saw_word = true;
    }

    saw_word
}

fn unwrap_redundant_parentheses(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut cursor = 0;

    while let Some(relative_start) = s.get(cursor..).and_then(|tail| tail.find("((")) {
        let start = cursor + relative_start;
        let Some(close_idx) = find_matching_closing_paren(s, start) else {
            break;
        };

        result.push_str(s.get(cursor..start).unwrap_or(""));
        result.push_str(s.get(start + 1..close_idx).unwrap_or(""));
        cursor = close_idx + 1;
    }

    result.push_str(s.get(cursor..).unwrap_or(""));
    result
}

fn find_matching_closing_paren(s: &str, open_idx: usize) -> Option<usize> {
    if !s.get(open_idx..)?.starts_with('(') {
        return None;
    }

    let mut depth = 0;
    for (offset, ch) in s.get(open_idx..)?.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open_idx + offset);
                }
            }
            _ => {}
        }
    }

    None
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
        if let Some((first_amount, first_consumed)) = parse_leading_amount_words(&words) {
            let range_connector = words
                .get(first_consumed)
                .filter(|word| word.eq_ignore_ascii_case("to") || word.eq_ignore_ascii_case("or"))
                .copied();
            if let Some(connector) = range_connector {
                let second_start = first_consumed + 1;
                if let Some((second_amount, second_consumed)) =
                    parse_leading_amount_words(&words[second_start..])
                {
                    let amount = format!("{} {} {}", first_amount, connector, second_amount);
                    let remaining_after_range = words[second_start + second_consumed..].join(" ");
                    return (Some(amount), remaining_after_range);
                }
            }
        }

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

        // Hyphenated range with attached mixed number on the high end:
        // "1-1 1/2 cups" → amount "1-1 1/2" (range from 1 to 1.5).
        // Low side may be digits or a fraction; high side (after the hyphen
        // in the first token) must be a non-empty digit run so we don't
        // swallow forms like "1/2-pound" or stray "1- " tokens.
        if first.contains('-') && !first.starts_with('-') && is_fraction(second) {
            if let Some((low, high_whole)) = first.split_once('-') {
                if is_amount_like(low)
                    && !high_whole.is_empty()
                    && high_whole.chars().all(|c| c.is_ascii_digit())
                {
                    let amount = format!("{} {}", first, second);
                    let remaining = words[2..].join(" ");
                    return (Some(amount), remaining);
                }
            }
        }

        // Hyphenated range with attached mixed number on the low end:
        // "1 1/2-2 cups" → amount "1 1/2-2"
        // "1 1/2-2 1/2 cups" → amount "1 1/2-2 1/2"
        if first.chars().all(|c| c.is_ascii_digit()) {
            if let Some((frac_low, after_hyphen)) = second.split_once('-') {
                if is_fraction(frac_low)
                    && !after_hyphen.is_empty()
                    && after_hyphen.chars().all(|c| c.is_ascii_digit())
                {
                    let (high, consumed) = if words.len() >= 3 && is_fraction(words[2]) {
                        (format!("{} {}", after_hyphen, words[2]), 3)
                    } else {
                        (after_hyphen.to_string(), 2)
                    };
                    let amount = format!("{} {}-{}", first, frac_low, high);
                    let remaining = words[consumed..].join(" ");
                    return (Some(amount), remaining);
                }
            }
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
        if words.len() >= 3 && words[1].eq_ignore_ascii_case("to") && is_amount_like(words[0]) {
            if is_amount_like(words[2]) {
                let amount = format!("{} to {}", words[0], words[2]);
                let remaining_after_range = words[3..].join(" ");
                return (Some(amount), remaining_after_range);
            }
            if let Some((attached_amount, attached_unit)) =
                split_leading_attached_unit_token(words[2])
            {
                let amount = format!("{} to {}", words[0], attached_amount);
                let trailing = words[3..].join(" ");
                let remaining = if trailing.is_empty() {
                    attached_unit.to_string()
                } else {
                    format!("{} {}", attached_unit, trailing)
                };
                return (Some(amount), remaining);
            }
        }

        // Check for range with "or": "3 or 4" (meaning 3-4, not alternatives)
        // This handles "3 or 4 drops of Tabasco" → amount="3 or 4", unit="drops"
        if words.len() >= 3 && words[1].eq_ignore_ascii_case("or") && is_amount_like(words[0]) {
            if is_amount_like(words[2]) {
                let amount = format!("{} or {}", words[0], words[2]);
                let remaining_after_range = words[3..].join(" ");
                return (Some(amount), remaining_after_range);
            }
            if let Some((attached_amount, attached_unit)) =
                split_leading_attached_unit_token(words[2])
            {
                let amount = format!("{} or {}", words[0], attached_amount);
                let trailing = words[3..].join(" ");
                let remaining = if trailing.is_empty() {
                    attached_unit.to_string()
                } else {
                    format!("{} {}", attached_unit, trailing)
                };
                return (Some(amount), remaining);
            }
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

/// Parse a leading amount from whitespace-split words.
/// Returns the normalized amount string and number of consumed words.
fn parse_leading_amount_words(words: &[&str]) -> Option<(String, usize)> {
    if words.is_empty() {
        return None;
    }

    let first = words[0];

    if words.len() >= 2 && first.chars().all(|c| c.is_ascii_digit()) && is_fraction(words[1]) {
        return Some((format!("{} {}", first, words[1]), 2));
    }

    if let Some((whole, fraction)) = split_hyphenated_mixed_number(first) {
        return Some((format!("{} {}", whole, fraction), 1));
    }

    if words.len() >= 3
        && first.chars().all(|c| c.is_ascii_digit())
        && (words[1].eq_ignore_ascii_case("and") || words[1] == "&")
        && is_fraction(words[2])
    {
        return Some((format!("{} {}", first, words[2]), 3));
    }

    if is_amount_like(first) {
        return Some((first.to_string(), 1));
    }

    None
}

fn split_hyphenated_mixed_number(s: &str) -> Option<(&str, &str)> {
    let (whole, fraction) = s.split_once('-')?;
    if whole.chars().all(|c| c.is_ascii_digit()) && is_fraction(fraction) {
        Some((whole, fraction))
    } else {
        None
    }
}

fn split_leading_attached_unit_token(s: &str) -> Option<(&str, &str)> {
    let mut split_idx = 0;
    let mut has_digit = false;
    let mut has_dot = false;

    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() {
            has_digit = true;
            split_idx = i + 1;
        } else if c == '.' && has_digit && !has_dot {
            has_dot = true;
            split_idx = i + 1;
        } else {
            break;
        }
    }

    if !has_digit || split_idx == 0 || split_idx >= s.len() {
        return None;
    }

    let amount = s.get(..split_idx)?;
    let unit = s.get(split_idx..)?.trim_end_matches('.');
    if unit.is_empty() {
        return None;
    }

    let normalized_unit = normalize_unit(unit);
    if ATTACHED_METRIC_UNITS.contains(&normalized_unit.as_str()) {
        Some((amount, unit))
    } else {
        None
    }
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
    // Hyphenated mixed number, e.g. "1-1/2"
    if split_hyphenated_mixed_number(s).is_some() {
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

fn units_share_base(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
        || a.strip_suffix(&format!(" {}", b))
            .is_some_and(|prefix| !prefix.is_empty())
        || b.strip_suffix(&format!(" {}", a))
            .is_some_and(|prefix| !prefix.is_empty())
}

fn parse_range_continuation_measurement(s: &str) -> Option<(String, Option<String>, String)> {
    let (pre_amount_modifier, after_modifier) = strip_measurement_modifier(s);
    let (mut amount, after_amount) = extract_amount(&after_modifier);
    let (pre_unit_modifier, after_pre_unit) = strip_measurement_modifier(&after_amount);
    let modifier = pre_unit_modifier.or(pre_amount_modifier);
    let (mut base_unit, mut remaining) = extract_unit(&after_pre_unit);

    if base_unit.is_none() {
        if let Some((descriptor_amount, descriptor_unit, descriptor_remaining)) =
            try_extract_hyphenated_descriptor_range(&after_pre_unit)
        {
            amount = Some(descriptor_amount);
            base_unit = Some(descriptor_unit);
            remaining = descriptor_remaining;
        } else if let Some(amount_str) = amount.as_deref() {
            if let Some(((replacement_amount, recovered_unit), after_recovered)) =
                try_extract_hyphenated_unit_tail(amount_str, &after_pre_unit)
            {
                amount = Some(replacement_amount);
                base_unit = Some(recovered_unit);
                remaining = after_recovered;
            }
        }
    }

    let unit = match (modifier, base_unit) {
        (Some(m), Some(u)) => Some(format!("{} {}", m, u)),
        (Some(m), None) => Some(m),
        (None, u) => u,
    };

    Some((amount?, unit, remaining))
}

fn try_extract_hyphenated_descriptor_range(s: &str) -> Option<(String, String, String)> {
    let s = s.trim();
    let words: Vec<&str> = s.split_whitespace().collect();
    let first = *words.first()?;
    let (amount, unit) = first.split_once('-')?;
    if !amount.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let normalized_unit = unit.to_lowercase();
    let normalized_unit = normalized_unit.trim_end_matches('.');
    if normalized_unit != "inch" && normalized_unit != "inches" {
        return None;
    }

    let descriptor = *words.get(1)?;
    let descriptor_lower = descriptor.to_lowercase();
    let descriptor_lower = descriptor_lower.trim_end_matches('.');
    if !HYPHENATED_DESCRIPTOR_NOUNS.contains(&descriptor_lower) {
        return None;
    }

    let remaining = words[2..].join(" ");
    let remaining = remaining.trim();
    let remaining_lower = remaining.to_lowercase();
    let remaining = if remaining_lower.starts_with("of ") || remaining_lower == "of" {
        remaining.get(2..).unwrap_or("").trim_start().to_string()
    } else {
        remaining.to_string()
    };

    Some((
        amount.to_string(),
        format!("{} {}", unit, descriptor),
        remaining,
    ))
}

/// Extract a unit from the beginning of a string.
/// Returns (unit, remaining_string).
fn slash_starts_measurement(s: &str) -> bool {
    let Some(after_slash) = s.strip_prefix('/') else {
        return false;
    };
    let after_slash = after_slash.trim_start();
    let (amount, _) = extract_amount(after_slash);
    amount.is_some()
}

fn extract_unit(s: &str) -> (Option<String>, String) {
    let s = s.trim();
    let s_lower = s.to_lowercase();

    for &unit in UNITS_SORTED.iter() {
        if s_lower.starts_with(unit) {
            // Make sure it's a word boundary
            let after = s.get(unit.len()..).unwrap_or("");
            if after.is_empty()
                || after.starts_with(|c: char| c.is_whitespace() || c == '.' || c == ',')
                || slash_starts_measurement(after)
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

/// Count/container nouns that pair with a hyphenated size descriptor.
/// Examples: "14-ounce package", "1/2-inch piece", "8-ounce block".
const HYPHENATED_DESCRIPTOR_NOUNS: &[&str] = &[
    "package", "packages", "pkg", "pkgs", "can", "cans", "bag", "bags", "block", "blocks", "wheel",
    "wheels", "piece", "pieces", "knob", "knobs", "segment", "segments", "slice", "slices",
    "stick", "sticks", "loaf", "loaves", "hunk", "hunks",
];

/// Recover a measurement from a hyphenated tail left behind after the numeric
/// amount has already been extracted, e.g. "-ounce can" or "/2-inch piece".
fn try_extract_hyphenated_unit_tail(amount: &str, s: &str) -> Option<((String, String), String)> {
    let s = s.trim();
    if s.is_empty() || (!s.starts_with('-') && !s.starts_with('/')) {
        return None;
    }

    let words: Vec<&str> = s.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }

    let reconstructed_first = format!("{}{}", amount, words[0]);
    let (_, after_hyphen) = reconstructed_first.split_once('-')?;
    let normalized_unit = after_hyphen.to_lowercase();
    let normalized_unit = normalized_unit.trim_end_matches('.');

    let is_known_base_unit = WEIGHT_UNITS_FOR_COMPOUND.contains(&normalized_unit)
        || normalized_unit == "inch"
        || normalized_unit == "inches";
    if !is_known_base_unit {
        return None;
    }

    let mut consumed_words = 1;
    let mut replacement_amount = amount.to_string();
    let mut unit = after_hyphen.to_string();

    if let Some(second) = words.get(1) {
        let second_normalized = second.to_lowercase();
        let second_normalized = second_normalized.trim_end_matches('.');
        if HYPHENATED_DESCRIPTOR_NOUNS.contains(&second_normalized) {
            consumed_words = 2;
            replacement_amount = "1".to_string();
            unit = format!("{} {}", reconstructed_first, second);
        }
    }

    let remaining = words[consumed_words..].join(" ");
    let remaining = remaining.trim();
    let remaining_lower = remaining.to_lowercase();
    let remaining = if remaining_lower.starts_with("of ") || remaining_lower == "of" {
        remaining.get(2..).unwrap_or("").trim_start().to_string()
    } else {
        remaining.to_string()
    };
    Some(((replacement_amount, unit), remaining))
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

/// Try to extract a measurement after a leading multiplier marker like "x" or "×".
/// Examples:
/// - "x 14 ounce cans black beans" -> ("14 ounce cans", "black beans")
/// - "x 400g cans cannellini beans" -> ("400g cans", "cannellini beans")
/// - "x packs shortcrust pastry" -> ("packs", "shortcrust pastry")
fn try_extract_multiplier_unit(s: &str) -> Option<(String, String)> {
    let s = s.trim_start();
    let after_marker = if let Some(rest) = s.strip_prefix('x') {
        rest
    } else if let Some(rest) = s.strip_prefix('×') {
        rest
    } else {
        return None;
    };

    let next_char = after_marker.chars().next()?;
    if !next_char.is_ascii_digit() && !next_char.is_whitespace() {
        return None;
    }

    let after_marker = after_marker.trim_start();
    if after_marker.is_empty() {
        return None;
    }

    if let Some((compound_unit, remaining)) = try_extract_compound_unit(after_marker) {
        return Some((compound_unit, remaining));
    }

    if let Some((measurement, after_metric)) = try_extract_attached_metric(after_marker) {
        let metric_amount = measurement.amount?;
        let metric_unit = measurement.unit?;
        let (container, remaining) = extract_unit(&after_metric);

        if let Some(container) = container {
            if CONTAINERS
                .iter()
                .any(|&c| c.eq_ignore_ascii_case(&container))
            {
                return Some((
                    format!("{}{} {}", metric_amount, metric_unit, container),
                    remaining,
                ));
            }
        }
    }

    let (unit, remaining) = extract_unit(after_marker);
    unit.map(|unit| (unit, remaining))
}

/// Check if a string looks like a preparation note.
fn is_prep_note(s: &str) -> bool {
    let s_lower = s.to_lowercase();
    PREP_NOTES.iter().any(|note| s_lower.contains(note))
}

fn is_trailing_prep_note(s: &str) -> bool {
    const PREP_FILLER_WORDS: &[&str] = &[
        "and", "but", "clean", "coarsely", "fine", "finely", "firmly", "freshly", "lightly",
        "loosely", "not", "or", "roughly", "small", "thinly", "very", "well",
    ];

    let mut saw_prep = false;
    for part in s.to_lowercase().split(',') {
        let part = part
            .trim()
            .trim_matches(|c: char| !c.is_ascii_alphabetic() && !c.is_ascii_whitespace());
        if part.is_empty() {
            continue;
        }
        if PREP_NOTES.contains(&part) {
            saw_prep = true;
            continue;
        }

        for word in part.split_whitespace() {
            let word = word.trim_matches(|c: char| !c.is_ascii_alphabetic());
            if word.is_empty() || PREP_FILLER_WORDS.contains(&word) {
                continue;
            }

            if PREP_NOTES
                .iter()
                .any(|note| word == *note || note.starts_with(&format!("{} ", word)))
            {
                saw_prep = true;
            } else {
                return false;
            }
        }
    }

    saw_prep
}

/// Check if a string consists only of prep words (comma-separated).
/// e.g., "finely chopped" -> true, "cooked chicken" -> false, "sliced" -> true
fn is_only_prep_words(s: &str) -> bool {
    let s_lower = s.to_lowercase();

    // Split by commas and check each part
    for part in s_lower.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        // Check if this part is entirely prep words
        let words: Vec<&str> = part.split_whitespace().collect();
        if words.is_empty() {
            continue;
        }

        let all_prep = words.iter().all(|word| {
            PREP_NOTES.iter().any(|note| {
                // Check if word matches note exactly or note starts with word
                *word == *note || note.starts_with(&format!("{} ", word))
            })
        });

        if !all_prep {
            return false;
        }
    }

    true
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

/// Equipment phrases that indicate a line is about kitchen tools, not ingredients.
/// Only compound phrases to avoid false positives (e.g., bare "pan" would match "pan spray").
const EQUIPMENT_PHRASES: &[&str] = &[
    "bundt pan",
    "cake pan",
    "tart pan",
    "roasting pan",
    "baking pan",
    "loaf pan",
    "sheet pan",
    "baking sheet",
    "cookie sheet",
    "muffin tin",
    "dutch oven",
    "slow cooker",
    "stand mixer",
    "hand mixer",
    "immersion blender",
    "food processor",
    "pastry bag",
    "piping bag",
    "pastry tip",
    "piping tip",
    "star tip",
    "saucepan",
    "stockpot",
    "skillet",
    "electric mixer",
];

/// Words that indicate a line is about food/ingredients even if it mentions equipment.
const INGREDIENT_INDICATOR_WORDS: &[&str] = &[
    "oil",
    "butter",
    "spray",
    "drippings",
    "grease",
    "greasing",
    "paper",
    "parchment",
    "wrap",
    "flour",
    "sugar",
    "salt",
    "water",
    "cream",
    "milk",
    "egg",
    "bread",
    "crumb",
    "seed",
    "nut",
    "walnut",
    "pistachio",
    "pecan",
    "crab",
    "meat",
    "pork",
    "beef",
    "chicken",
    "turkey",
    "cheese",
    "onion",
    "garlic",
    "pepper",
    "tomato",
    "carrot",
    "coconut",
    "lard",
    "frosting",
    "batter",
    "dough",
    "beet",
];

/// Check if a word appears at a word boundary in the text.
/// The word must start at a word boundary (beginning of text or preceded by non-alpha char),
/// but may be followed by additional letters (to allow plurals like "seeds" matching "seed").
fn contains_word_prefix(text: &str, word: &str) -> bool {
    for (i, _) in text.match_indices(word) {
        let before_ok = i == 0 || !text.as_bytes()[i - 1].is_ascii_alphabetic();
        if before_ok {
            return true;
        }
    }
    false
}

/// Check if a line describes kitchen equipment rather than an ingredient.
fn is_equipment_line(lower: &str) -> bool {
    // Equipment phrases can match as substrings (they're multi-word and specific enough)
    if !EQUIPMENT_PHRASES.iter().any(|&eq| lower.contains(eq)) {
        return false;
    }
    // Ingredient indicators must match as whole words to avoid false matches
    // (e.g., "foil" should not match "oil")
    if INGREDIENT_INDICATOR_WORDS
        .iter()
        .any(|&ing| contains_word_prefix(lower, ing))
    {
        return false;
    }
    true
}

/// Check if a line should be completely ignored (scraper artifact or equipment).
/// Returns true if the line should be skipped entirely.
pub fn should_ignore_line(raw: &str) -> bool {
    let trimmed = raw.trim();
    let lower = trimmed.to_lowercase();

    // Skip lines that are only asterisks (footnote section dividers, e.g., "**")
    if !trimmed.is_empty() && trimmed.chars().all(|c| c == '*') {
        return true;
    }

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

    // Check if the line is purely about equipment/tools
    if is_equipment_line(&lower) {
        return true;
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

    // Pattern 2b: Imperative "To X" / "To the X" patterns
    // ("To assemble:", "To serve:", "To finish:", "To assemble the cake:")
    if name_lower.starts_with("to ") {
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

    // Pattern 4: Short colon-terminated headers with no digits.
    // We already rejected anything that parses with both amount + unit above,
    // so by here the line has no obvious ingredient shape. Treat short,
    // digit-free, few-word phrases as section headers (e.g. "Dough:",
    // "Asparagus pesto:", "Chicken and noodle salad:").
    let word_count = name.split_whitespace().count();
    if name.len() <= 50 && word_count <= 5 && !name.chars().any(|c| c.is_ascii_digit()) {
        return Some(normalize_section_name(name));
    }

    // Pattern 5: Longer mixed-case headers containing a well-known section
    // keyword. Short phrases are already covered by pattern 4; this fallback
    // catches verbose labels like "Creamy Artichoke Spread (makes a little
    // extra):" or "PART III: The ham and nut filling:" where the word count
    // exceeds five but a section keyword gives us high confidence.
    if name.len() <= 80 {
        const SECTION_KEYWORDS: &[&str] = &[
            "topping", "filling", "frosting", "icing", "glaze", "sauce", "marinade", "dressing",
            "crust", "batter", "drizzle", "garnish", "assembly", "serving", "optional", "coating",
            "base", "cream", "streusel", "crumble", "spread",
        ];
        if SECTION_KEYWORDS.iter().any(|kw| name_lower.contains(kw)) {
            return Some(normalize_section_name(name));
        }
    }

    None
}

/// Split a compound item string into individual items.
///
/// Splitting rules:
/// 1. Replace ", and " with ", " (normalize Oxford comma)
/// 2. Split on ", "
/// 3. Further split each part on " and "
/// 4. Trim each result, drop empties
fn split_compound_items(item: &str) -> Vec<String> {
    // Normalize Oxford commas
    let normalized = item.replace(", and ", ", ");

    let mut result = Vec::new();
    for part in normalized.split(", ") {
        for sub in part.split(" and ") {
            let trimmed = sub.trim();
            if !trimmed.is_empty() {
                result.push(trimmed.to_string());
            }
        }
    }

    result
}

/// Expand a parsed ingredient with "each" modifier into multiple ingredients.
///
/// When the first measurement's unit ends with " each" and the item contains
/// comma/and-separated items, split into one ParsedIngredient per sub-item,
/// each with the same measurements but " each" stripped from units.
///
/// Only checks the first measurement to avoid false positives on parenthetical
/// per-unit weights like "4 chicken breasts (8 oz each)".
///
/// Returns a vec with a single element (the original) if no expansion is needed.
pub fn expand_each_ingredients(ingredient: ParsedIngredient) -> Vec<ParsedIngredient> {
    // Check: does the FIRST measurement have a unit ending in " each"?
    let first_has_each = ingredient
        .measurements
        .first()
        .and_then(|m| m.unit.as_ref())
        .is_some_and(|u| u.ends_with(" each"));
    if !first_has_each {
        return vec![ingredient];
    }

    // Split the item on delimiters
    let sub_items = split_compound_items(&ingredient.item);
    if sub_items.len() <= 1 {
        return vec![ingredient];
    }

    // Build stripped measurements (remove " each" suffix from all units)
    let stripped_measurements: Vec<Measurement> = ingredient
        .measurements
        .iter()
        .map(|m| Measurement {
            amount: m.amount.clone(),
            unit: m
                .unit
                .as_ref()
                .map(|u| u.strip_suffix(" each").unwrap_or(u).to_string()),
        })
        .collect();

    // Create one ParsedIngredient per sub-item
    sub_items
        .into_iter()
        .map(|sub_item| ParsedIngredient {
            item: sub_item,
            measurements: stripped_measurements.clone(),
            note: ingredient.note.clone(),
            raw: ingredient.raw.clone(),
            section: ingredient.section.clone(),
        })
        .collect()
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
        // Expand "each" compound ingredients (e.g., "1/2 tsp each salt and pepper" -> 2 ingredients)
        results.extend(expand_each_ingredients(ingredient));
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
    fn test_detect_section_header_long_keyword_fallback() {
        // Headers longer than five words still count as sections when they
        // contain a well-known section keyword.
        assert!(detect_section_header("Creamy Artichoke Spread (makes a little extra):").is_some());
        assert!(detect_section_header("PART III: The ham and nut filling:").is_some());
        // A long phrase without any recognized keyword stays an ingredient
        // (we don't have enough signal to call it a header).
        assert_eq!(
            detect_section_header("This is a long phrase with no keyword at all:"),
            None
        );
    }

    #[test]
    fn test_detect_section_header_to_assemble_pattern() {
        // Imperative "To X:" headers (parallel to "For X:") should be detected
        assert_eq!(
            detect_section_header("To assemble:"),
            Some("To Assemble".to_string())
        );
        assert_eq!(
            detect_section_header("To serve:"),
            Some("To Serve".to_string())
        );
        assert_eq!(
            detect_section_header("To finish:"),
            Some("To Finish".to_string())
        );
        assert_eq!(
            detect_section_header("To assemble the cake:"),
            Some("To Assemble the Cake".to_string())
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
    fn test_should_ignore_equipment_lines() {
        // Equipment-only lines should be ignored
        assert!(should_ignore_line(
            "A 12-cup Bundt pan, a pastry bag, and a large star tip"
        ));
        assert!(should_ignore_line(
            "A 9½\"-diameter tart pan with removable bottom"
        ));
        assert!(should_ignore_line(
            "Large dutch oven, pot, or high-sided skillet."
        ));
        assert!(should_ignore_line(
            "A dutch oven or large heavy-bottomed pot"
        ));
        assert!(should_ignore_line(
            "9  \" wide cake pan with 2\" tall sides."
        ));
        assert!(should_ignore_line(
            "2 9x13 metal baking pans or (1) large roasting pan lined with foil."
        ));
        assert!(should_ignore_line("11.38 x 16.5 lipped baking pan"));
        assert!(should_ignore_line("6 QT Slow Cooker"));
        assert!(should_ignore_line(
            "Hand immersion blender (preferred & easier, or food processor, or blender)"
        ));
        assert!(should_ignore_line("stand mixer"));
        assert!(should_ignore_line("food processor"));
        assert!(should_ignore_line(
            "1 large disposable aluminum roasting pan (if using charcoal)"
        ));

        // Ingredient lines mentioning equipment should NOT be ignored
        assert!(!should_ignore_line("Pan drippings from roasting pan"));
        assert!(!should_ignore_line("oil, for the pan"));
        assert!(!should_ignore_line("butter for the pan"));
        assert!(!should_ignore_line("pan spray"));
        assert!(!should_ignore_line("Parchment paper"));
        assert!(!should_ignore_line(
            "4 18×13-inch (approximately) pieces parchment paper"
        ));
        assert!(!should_ignore_line(
            "8  16-inch long pieces of Reynolds Wrap Heavy Duty Foil"
        ));
        assert!(!should_ignore_line("Vegetable oil, for greasing pan"));
        assert!(!should_ignore_line("2 cups pan drippings liquid"));
        assert!(!should_ignore_line(
            "ghee (butter or coconut oil (to coat skillet))"
        ));
        assert!(!should_ignore_line(
            "2 cups panko breadcrumbs, pulsed in a food processor"
        ));
        assert!(!should_ignore_line(
            "1 tsp caraway seeds, (toasted and ground (use food processor))"
        ));
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

    #[test]
    fn test_is_only_prep_words() {
        // Single prep words
        assert!(is_only_prep_words("sliced"));
        assert!(is_only_prep_words("chopped"));
        assert!(is_only_prep_words("cooked"));
        assert!(is_only_prep_words("toasted"));
        assert!(is_only_prep_words("roasted"));

        // Multiple prep words
        assert!(is_only_prep_words("finely chopped"));
        assert!(is_only_prep_words("thinly sliced"));

        // Not only prep words (contains actual ingredient)
        assert!(!is_only_prep_words("cooked chicken"));
        assert!(!is_only_prep_words("chicken"));
        assert!(!is_only_prep_words("sliced onions"));

        // Edge cases
        assert!(is_only_prep_words("")); // empty returns true (no non-prep words)
        assert!(!is_only_prep_words("salt"));
        assert!(!is_only_prep_words("butter"));
    }

    #[test]
    fn test_trailing_prep_note_requires_prep_phrase_not_noun_phrase() {
        assert!(is_trailing_prep_note("finely chopped"));
        assert!(is_trailing_prep_note("peeled and chopped"));
        assert!(is_trailing_prep_note("chopped very small"));
        assert!(is_trailing_prep_note("scrubbed clean"));
        assert!(is_trailing_prep_note("scrubbed clean but not peeled"));
        assert!(!is_trailing_prep_note("unseasoned dried breadcrumbs"));
        assert!(!is_trailing_prep_note("cooked chicken meat"));
    }

    #[test]
    fn test_orphaned_prep_word_fallback() {
        // When the item would be just a prep word, return raw
        let result = parse_ingredient("sliced");
        assert_eq!(result.item, "sliced");

        // When comma is between prep words and ingredient, don't split there
        // "2 cups finely chopped, cooked chicken meat" should NOT extract
        // "cooked chicken meat" as note, leaving "finely chopped" as item
        let result = parse_ingredient("2 cups finely chopped, cooked chicken meat");
        // Item should include the ingredient, not just prep words
        assert_eq!(result.item, "finely chopped, cooked chicken meat");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_parenthetical_note_does_not_expose_comma_noun_phrase_to_prep_strip() {
        let result = parse_ingredient(
            "3/4 cup (40 grams) plain, unseasoned dried breadcrumbs (I used, and recommend, panko, see note above)",
        );
        assert_eq!(result.item, "plain, unseasoned dried breadcrumbs");
        assert_eq!(
            result.note,
            Some("I used, and recommend, panko, see note above".to_string())
        );
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("3/4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
        assert_eq!(result.measurements[1].amount, Some("40".to_string()));
        assert_eq!(result.measurements[1].unit, Some("g".to_string()));
    }

    #[test]
    fn test_trailing_scrubbed_clean_is_note_not_item_identity() {
        let result = parse_ingredient("4 russet potatoes, scrubbed clean");
        assert_eq!(result.item, "russet potatoes");
        assert_eq!(result.note, Some("scrubbed clean".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("4".to_string()));
        assert_eq!(result.measurements[0].unit, None);
    }

    #[test]
    fn test_conditional_alternate_keeps_parenthetical_condition_with_amount() {
        let result = parse_ingredient(
            "4 cups vegetable broth (for dried but soaked chickpeas), 1 1/2 cups vegetable broth (for cooked chickpeas)",
        );
        assert_eq!(result.item, "vegetable broth");
        assert_eq!(
            result.note,
            Some("for dried but soaked chickpeas; or 1 1/2 cup for cooked chickpeas".to_string())
        );
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_hyphenated_compound_unit_tail_package() {
        let result = parse_ingredient("14-ounce package extra-firm tofu");
        assert_eq!(result.item, "extra-firm tofu");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(
            result.measurements[0].unit,
            Some("14-ounce package".to_string())
        );
    }

    #[test]
    fn test_hyphenated_compound_unit_tail_fractional_piece() {
        let result = parse_ingredient("1/2-inch piece of fresh ginger");
        assert_eq!(result.item, "fresh ginger");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(
            result.measurements[0].unit,
            Some("1/2-inch piece".to_string())
        );
    }

    #[test]
    fn test_unicode_fraction_slash_mixed_number() {
        let result = parse_ingredient("1 1⁄2 cups crushed kettle-style potato chips");
        assert_eq!(result.item, "crushed kettle-style potato chips");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1 1/2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_hyphenated_parenthetical_measurement_preserves_unit() {
        let result = parse_ingredient("1-pound (454-gram) package phyllo/filo pastry");
        assert_eq!(result.item, "phyllo/filo pastry");
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(
            result.measurements[0].unit,
            Some("1-pound package".to_string())
        );
        assert_eq!(result.measurements[1].amount, Some("454".to_string()));
        assert_eq!(result.measurements[1].unit, Some("g".to_string()));
    }

    #[test]
    fn test_parenthetical_yield_hint_does_not_create_alt_measurement() {
        let result = parse_ingredient("1/4 cup freshly squeezed lemon juice (2 lemons)");
        assert_eq!(result.item, "freshly squeezed lemon juice");
        assert_eq!(result.note, Some("2 lemons".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1/4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_parenthetical_slash_separated_weight_alternates_are_preserved() {
        let result = parse_ingredient("1 cup (5 ounces/142 grams) all-purpose flour");
        assert_eq!(result.item, "all-purpose flour");
        assert_eq!(result.measurements.len(), 3);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
        assert_eq!(result.measurements[1].amount, Some("5".to_string()));
        assert_eq!(result.measurements[1].unit, Some("oz".to_string()));
        assert_eq!(result.measurements[2].amount, Some("142".to_string()));
        assert_eq!(result.measurements[2].unit, Some("g".to_string()));
    }

    #[test]
    fn test_parenthetical_compact_metric_range_is_preserved() {
        let result = parse_ingredient("1 to 2 tablespoons (15 to 30g) unsalted butter");
        assert_eq!(result.item, "unsalted butter");
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("1 to 2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("tbsp".to_string()));
        assert_eq!(result.measurements[1].amount, Some("15 to 30".to_string()));
        assert_eq!(result.measurements[1].unit, Some("g".to_string()));
    }

    #[test]
    fn test_parenthetical_size_hint_becomes_note() {
        let result = parse_ingredient("12 (6-inch) flour tortillas, warmed");
        assert_eq!(result.item, "flour tortillas, warmed");
        assert_eq!(result.note, Some("6-inch".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("12".to_string()));
        assert_eq!(result.measurements[0].unit, None);
    }

    #[test]
    fn test_parenthetical_package_size_is_not_treated_as_recipe_amount() {
        let result = parse_ingredient(
            "1/2 cup cooked black beans from one (15-ounce) can, drained and rinsed",
        );
        assert_eq!(result.item, "cooked black beans from one can");
        assert_eq!(
            result.note,
            Some("15-ounce, drained and rinsed".to_string())
        );
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1/2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_parenthetical_temperature_hint_becomes_note() {
        let result = parse_ingredient("1 3/4 cups warm water (about 100 degrees)");
        assert_eq!(result.item, "warm water");
        assert_eq!(result.note, Some("about 100 degrees".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1 3/4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_parenthetical_temperature_hint_preserves_trailing_note() {
        let result = parse_ingredient(
            "2.5 cups warm water (85 degrees F), divided, plus more as needed for feeding",
        );
        assert_eq!(result.item, "warm water, divided");
        assert_eq!(
            result.note,
            Some("85 degrees F, plus more as needed for feeding".to_string())
        );
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("2.5".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_nested_parenthetical_note_is_preserved() {
        let result = parse_ingredient("2 tbsp milk (, full fat (low fat ok too))");
        assert_eq!(result.item, "milk");
        assert_eq!(result.note, Some("full fat (low fat ok too)".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("tbsp".to_string()));
    }

    #[test]
    fn test_parenthetical_item_identity_is_preserved_as_item_text() {
        let result = parse_ingredient("3 large (ripe bananas, well mashed (about 1 1/2 cups))");
        assert_eq!(result.item, "ripe bananas");
        assert_eq!(
            result.note,
            Some("well mashed (about 1 1/2 cups)".to_string())
        );
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("3".to_string()));
        assert_eq!(result.measurements[0].unit, Some("large".to_string()));
    }

    #[test]
    fn test_parenthetical_item_identity_allows_punctuation() {
        let result = parse_ingredient("3 cloves ((the spice cloves!))");
        assert_eq!(result.item, "the spice cloves!");
        assert_eq!(result.note, None);
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("3".to_string()));
        assert_eq!(result.measurements[0].unit, Some("clove".to_string()));
    }

    #[test]
    fn test_parenthetical_item_identity_promotes_after_color_descriptor() {
        let result = parse_ingredient("2 red (yellow or orange bell peppers, stemmed and seeded)");
        assert_eq!(result.item, "red yellow or orange bell peppers");
        assert_eq!(result.note, Some("stemmed and seeded".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("2".to_string()));
        assert_eq!(result.measurements[0].unit, None);
    }

    #[test]
    fn test_parenthetical_item_identity_promotes_after_color_with_unit() {
        let result = parse_ingredient("3 slices white (or wheat sandwich bread, crusts removed)");
        assert_eq!(result.item, "white or wheat sandwich bread");
        assert_eq!(result.note, Some("crusts removed".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("3".to_string()));
        assert_eq!(result.measurements[0].unit, Some("slice".to_string()));
    }

    #[test]
    fn test_parenthetical_approximate_quantity_guidance_becomes_note() {
        let result = parse_ingredient("5 cups thinly sliced cabbage (about half a small head)");
        assert_eq!(result.item, "thinly sliced cabbage");
        assert_eq!(result.note, Some("about half a small head".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("5".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_parenthetical_approximate_yield_hint_becomes_note() {
        let result = parse_ingredient("1 rack beef back ribs (about 7 bones)");
        assert_eq!(result.item, "rack beef back ribs");
        assert_eq!(result.note, Some("about 7 bones".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, None);
    }

    #[test]
    fn test_slash_separated_metric_alternate_after_primary_unit() {
        let result = parse_ingredient("1 3/4 cups/420 milliliters warm water (about 100 degrees)");
        assert_eq!(result.item, "warm water");
        assert_eq!(result.note, Some("about 100 degrees".to_string()));
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("1 3/4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
        assert_eq!(result.measurements[1].amount, Some("420".to_string()));
        assert_eq!(result.measurements[1].unit, Some("ml".to_string()));
    }

    #[test]
    fn test_slash_separated_metric_alternate_preserves_item_text() {
        let result = parse_ingredient("1 3/4 cups/225 grams all-purpose or bread flour");
        assert_eq!(result.item, "all-purpose or bread flour");
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("1 3/4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
        assert_eq!(result.measurements[1].amount, Some("225".to_string()));
        assert_eq!(result.measurements[1].unit, Some("g".to_string()));
    }

    #[test]
    fn test_slash_inside_descriptor_is_not_treated_as_measurement_boundary() {
        let result = parse_ingredient("4 or 5 small/medium zucchini/squash (about 2 1/2 lbs)");
        assert_eq!(result.item, "small/medium zucchini/squash");
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("4 or 5".to_string()));
        assert_eq!(result.measurements[0].unit, None);
        assert_eq!(result.measurements[1].amount, Some("2 1/2".to_string()));
        assert_eq!(result.measurements[1].unit, Some("lb".to_string()));
    }

    #[test]
    fn test_hyphenated_unit_tail_without_descriptor_noun() {
        let result = parse_ingredient("1-pound ground beef");
        assert_eq!(result.item, "ground beef");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("lb".to_string()));
    }

    #[test]
    fn test_split_compound_items() {
        assert_eq!(
            split_compound_items("salt and pepper"),
            vec!["salt", "pepper"]
        );
        assert_eq!(
            split_compound_items("dried oregano and cumin"),
            vec!["dried oregano", "cumin"]
        );
        assert_eq!(
            split_compound_items("ground cinnamon, ginger, cloves, and cardamom"),
            vec!["ground cinnamon", "ginger", "cloves", "cardamom"]
        );
        assert_eq!(
            split_compound_items("chili powder, onion powder, and garlic powder"),
            vec!["chili powder", "onion powder", "garlic powder"]
        );
        assert_eq!(
            split_compound_items("garlic and onion powder"),
            vec!["garlic", "onion powder"]
        );
        // Single item - no split
        assert_eq!(
            split_compound_items("pork tenderloins"),
            vec!["pork tenderloins"]
        );
    }

    #[test]
    fn test_expand_each_ingredients_basic() {
        let ingredient = ParsedIngredient {
            item: "salt and pepper".to_string(),
            measurements: vec![Measurement {
                amount: Some("0.5".to_string()),
                unit: Some("tsp each".to_string()),
            }],
            note: None,
            raw: Some("1/2 tsp each salt and pepper".to_string()),
            section: None,
        };
        let expanded = expand_each_ingredients(ingredient);
        assert_eq!(expanded.len(), 2);
        assert_eq!(expanded[0].item, "salt");
        assert_eq!(expanded[0].measurements[0].unit, Some("tsp".to_string()));
        assert_eq!(expanded[1].item, "pepper");
        assert_eq!(expanded[1].measurements[0].unit, Some("tsp".to_string()));
    }

    #[test]
    fn test_expand_each_no_split_parenthetical() {
        // "lb each" in measurement[1] with no compound item - should NOT split
        let ingredient = ParsedIngredient {
            item: "pork tenderloins".to_string(),
            measurements: vec![
                Measurement {
                    amount: Some("2".to_string()),
                    unit: None,
                },
                Measurement {
                    amount: Some("1".to_string()),
                    unit: Some("lb each".to_string()),
                },
            ],
            note: None,
            raw: Some("2 (1 lb each) pork tenderloins".to_string()),
            section: None,
        };
        let expanded = expand_each_ingredients(ingredient);
        assert_eq!(expanded.len(), 1);
        assert_eq!(expanded[0].item, "pork tenderloins");
    }

    #[test]
    fn test_expand_each_no_each_unit() {
        // No "each" in any unit - should NOT split even with "and" in item
        let ingredient = ParsedIngredient {
            item: "salt and pepper".to_string(),
            measurements: vec![Measurement {
                amount: Some("1".to_string()),
                unit: Some("tsp".to_string()),
            }],
            note: None,
            raw: Some("1 tsp salt and pepper".to_string()),
            section: None,
        };
        let expanded = expand_each_ingredients(ingredient);
        assert_eq!(expanded.len(), 1);
    }

    #[test]
    fn test_expand_each_four_items() {
        let ingredient = ParsedIngredient {
            item: "ground cinnamon, ginger, cloves, and cardamom".to_string(),
            measurements: vec![Measurement {
                amount: Some("0.5".to_string()),
                unit: Some("tsp each".to_string()),
            }],
            note: None,
            raw: Some(
                "1/2 teaspoon each ground cinnamon, ginger, cloves, and cardamom".to_string(),
            ),
            section: None,
        };
        let expanded = expand_each_ingredients(ingredient);
        assert_eq!(expanded.len(), 4);
        assert_eq!(expanded[0].item, "ground cinnamon");
        assert_eq!(expanded[1].item, "ginger");
        assert_eq!(expanded[2].item, "cloves");
        assert_eq!(expanded[3].item, "cardamom");
        for ing in &expanded {
            assert_eq!(ing.measurements[0].unit, Some("tsp".to_string()));
        }
    }

    #[test]
    fn test_parse_ingredients_expands_each() {
        let blob = "1/2 tsp each salt and pepper";
        let result = parse_ingredients(blob);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].item, "salt");
        assert_eq!(result[0].measurements[0].unit, Some("tsp".to_string()));
        assert_eq!(result[1].item, "pepper");
        assert_eq!(result[1].measurements[0].unit, Some("tsp".to_string()));
    }

    #[test]
    fn test_strip_trailing_asterisks_from_item() {
        // Single asterisk
        let result = parse_ingredient("3/4 cup (170g) unsalted butter*");
        assert_eq!(result.item, "unsalted butter");

        // Double asterisk
        let result = parse_ingredient("2/3 cup (56g) unsweetened cocoa powder**");
        assert_eq!(result.item, "unsweetened cocoa powder");

        // Triple asterisk
        let result = parse_ingredient("3/4 cup (128g) chocolate chips***");
        assert_eq!(result.item, "chocolate chips");

        // Asterisk with trailing comma (no note after)
        let result = parse_ingredient("7 tablespoons vegetable oil*,");
        assert_eq!(result.item, "vegetable oil");
    }

    #[test]
    fn test_should_ignore_standalone_asterisks() {
        assert!(should_ignore_line("**"));
        assert!(should_ignore_line("*"));
        assert!(should_ignore_line("***"));
        // Regular ingredients should not be ignored
        assert!(!should_ignore_line("1 cup flour*"));
    }

    #[test]
    fn test_strip_asterisks_from_note() {
        let result = parse_ingredient("1 cup butter, at room temperature**");
        assert_eq!(result.item, "butter");
        assert_eq!(result.note.as_deref(), Some("at room temperature"));
    }

    #[test]
    fn test_parse_ingredients_filters_standalone_asterisks() {
        let blob = "For pistou\n**\n3/4 cup fresh mint leaves";
        let result = parse_ingredients(blob);
        // The ** line should be filtered out
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].item, "fresh mint leaves");
    }

    #[test]
    fn test_mixed_number_range_high_end_attached() {
        // "1-1 1/2 cups …" must parse the whole "1-1 1/2" as the amount,
        // not split it as "1-1" amount + "1/2 cups …" item.
        let raw = "1-1 1/2 cups grilled chicken, cubed or shredded";
        let result = parse_ingredient(raw);
        assert_eq!(result.item, "grilled chicken");
        assert_eq!(result.note.as_deref(), Some("cubed or shredded"));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1-1 1/2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));

        let normalized = parse_ingredient(raw).normalize_amounts();
        assert_eq!(normalized.measurements[0].amount, Some("1-1.5".to_string()));
    }

    #[test]
    fn test_mixed_number_range_low_end_attached() {
        // "1 1/2-2 cups …" must parse "1 1/2-2" as the amount.
        let raw = "1 1/2-2 cups flour";
        let result = parse_ingredient(raw);
        assert_eq!(result.item, "flour");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1 1/2-2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));

        let normalized = parse_ingredient(raw).normalize_amounts();
        assert_eq!(normalized.measurements[0].amount, Some("1.5-2".to_string()));
    }

    #[test]
    fn test_mixed_number_range_both_ends_attached() {
        // "1 1/2-2 1/2 tablespoons olive oil" should yield amount "1 1/2-2 1/2".
        let result = parse_ingredient("1 1/2-2 1/2 tablespoons olive oil");
        assert_eq!(result.item, "olive oil");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(
            result.measurements[0].amount,
            Some("1 1/2-2 1/2".to_string())
        );
        assert_eq!(result.measurements[0].unit, Some("tbsp".to_string()));
    }

    #[test]
    fn test_mixed_number_range_low_attached_pound_unit_unaffected() {
        // "1 1/2-pound salmon fillet": after-hyphen is "pound" (not digits),
        // so the new low-end range branch must NOT fire and steal "pound"
        // into a numeric range. Whatever the rest of the parser does with
        // this input, the amount must not become a "1 1/2-pound" range.
        let result = parse_ingredient("1 1/2-pound salmon fillet");
        let amount = result.measurements[0].amount.as_deref().unwrap_or("");
        assert!(
            !amount.contains("pound"),
            "amount should not absorb 'pound' as part of a range, got {:?}",
            result.measurements[0]
        );
    }

    #[test]
    fn test_simple_hyphenated_range_still_works() {
        // Regression: plain integer hyphen ranges must still parse normally.
        let result = parse_ingredient("6-8 cups water");
        assert_eq!(result.item, "water");
        assert_eq!(result.measurements[0].amount, Some("6-8".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_hyphenated_mixed_number_still_works() {
        // Regression: "1-1/2" (no space, fraction after hyphen) is the
        // existing hyphenated-mixed-number form. The new branches must not
        // steal or transform it; behaviour matches pre-existing pipeline
        // fixtures (amount stays as "1-1/2").
        let result = parse_ingredient("1-1/2 cups sugar");
        assert_eq!(result.item, "sugar");
        assert_eq!(result.measurements[0].amount, Some("1-1/2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }
}
