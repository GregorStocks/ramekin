//! Ingredient parsing module.
//!
//! Parses raw ingredient strings (e.g., "2 cups flour, sifted") into structured data.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::metric_weights::parse_amount;
use crate::text::decode_html_entities;

mod amounts;
mod citrus;
mod classify;
mod fractions;
mod item;
mod measure;
mod parentheticals;
pub mod quantity;
mod segments;
mod tokens;
mod units;

pub(crate) use amounts::normalize_fraction_to_decimal;
use amounts::*;
pub(crate) use citrus::citrus_part_name;
use citrus::parse_citrus_part_line;
use item::*;
use parentheticals::*;
use segments::{Segments, Sep};
use units::*;

pub(in crate::ingredient_parser) use fractions::unicode_fraction_ascii;
pub(crate) use fractions::unicode_fraction_regex_class;
pub use item::{
    detect_section_header, normalize_unicode, should_ignore_line, strip_leading_list_marker,
};

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
    follows_or: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct ParentheticalMeasurementParse {
    measurements: Vec<Measurement>,
    has_non_measurement_content: bool,
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

fn format_measurement_segment(amount: &str, unit: &str) -> String {
    for modifier in MEASUREMENT_MODIFIERS {
        if let Some(base_unit) = unit.strip_prefix(&format!("{} ", modifier)) {
            return format!("{} {} {}", modifier, amount, base_unit);
        }
    }
    format!("{} {}", amount, unit)
}

fn format_measurement_segment_with_prefix(
    amount: &str,
    unit: &str,
    prefix: Option<&str>,
) -> String {
    match prefix {
        Some(prefix) => format!("{} {} {}", prefix, amount, unit),
        None => format_measurement_segment(amount, unit),
    }
}

fn strip_plus_measurement_prefix(s: &str) -> (Option<String>, String) {
    let s = s.trim();
    let lower = s.to_lowercase();
    for prefix in ["about", "approximately"] {
        if lower.starts_with(prefix) {
            if let Some(after) = s.get(prefix.len()..) {
                if after.is_empty() || after.starts_with(char::is_whitespace) {
                    return (Some(prefix.to_string()), after.trim_start().to_string());
                }
            }
        }
    }
    (None, s.to_string())
}

fn parse_plus_continuation_measurement(
    s: &str,
) -> Option<(String, String, String, Option<String>)> {
    let (plus_prefix, after_plus_prefix) = strip_plus_measurement_prefix(s);
    let read = measure::read_measurement(&after_plus_prefix);
    Some((
        read.amount?,
        read.unit?,
        read.rest,
        read.modifier.or(plus_prefix),
    ))
}

fn leading_measurement_consumes_all(s: &str) -> bool {
    let read = measure::read_measurement(s);
    read.amount.is_some() && read.unit.is_some() && read.rest.is_empty()
}

fn strip_leading_measurement_qualifier(s: &str) -> (Option<&str>, &str) {
    let trimmed = s.trim_start();
    let lower = trimmed.to_lowercase();
    for qualifier in ["about", "approximately", "approx.", "approx", "roughly"] {
        if lower.starts_with(qualifier) {
            if let Some(after) = trimmed.get(qualifier.len()..) {
                if after.is_empty() || after.starts_with(char::is_whitespace) {
                    return (Some(qualifier), after.trim_start());
                }
            }
        }
    }
    (None, trimmed)
}

fn starts_with_measurement_fragment(s: &str) -> bool {
    let (_, after_qualifier) = strip_leading_measurement_qualifier(s);
    let (_, after_pre_amount_modifier) = strip_measurement_modifier(after_qualifier);
    let (amount, after_amount) = extract_amount(&after_pre_amount_modifier);
    amount.is_some() && !after_amount.trim().is_empty()
}

fn extract_unit_allowing_attached_item(s: &str) -> (Option<String>, String) {
    let (unit, after_unit) = extract_unit(s);
    if unit.is_some() {
        return (unit, after_unit);
    }

    let trimmed = s.trim_start();
    let lower = trimmed.to_lowercase();
    for unit in UNITS_SORTED
        .iter()
        .copied()
        .filter(|unit| unit.len() > 1 && !is_count_noun(unit))
    {
        if !lower.starts_with(unit) {
            continue;
        }
        let Some(after_unit) = trimmed.get(unit.len()..) else {
            continue;
        };
        let Some(next_char) = after_unit.chars().next() else {
            continue;
        };
        if next_char.is_ascii_alphabetic() {
            return (
                Some(trimmed.get(..unit.len()).unwrap_or(unit).to_string()),
                after_unit.to_string(),
            );
        }
    }

    (None, trimmed.to_string())
}

fn split_malformed_parenthetical_measurement_item(s: &str) -> Option<(String, String)> {
    let (qualifier, after_qualifier) = strip_leading_measurement_qualifier(s);
    let (amount, after_amount) = extract_amount(after_qualifier);
    let amount = amount?;
    let (unit, after_unit) = extract_unit_allowing_attached_item(&after_amount);
    let unit = unit?;
    let item = after_unit.trim().trim_start_matches(',').trim();
    if item.is_empty() || !looks_like_parenthetical_item_identity(item) {
        return None;
    }

    let measurement = match qualifier {
        Some(qualifier) => format!("{} {} {}", qualifier, amount, unit),
        None => format!("{} {}", amount, unit),
    };
    Some((measurement, item.to_string()))
}

fn split_malformed_parenthetical_package_item(s: &str) -> Option<(String, String)> {
    let (qualifier, after_qualifier) = strip_leading_measurement_qualifier(s);
    if qualifier.is_some() {
        return None;
    }

    // "14-ounce can chickpeas": one package of a hyphenated size.
    let (amount, unit, after_unit) = measure::read_hyphenated_size(after_qualifier)?;
    if amount != "1" || !unit.contains(' ') {
        return None;
    }

    let item = after_unit.trim_start_matches(',').trim();
    let identity_segment = item.split(" or ").next().unwrap_or(item).trim();
    if item.is_empty() || !looks_like_parenthetical_item_identity(identity_segment) {
        return None;
    }

    Some((unit, item.to_string()))
}

fn repair_unclosed_parenthetical_measurement_fragment(s: &str) -> String {
    let Some(start) = tokens::first_group(s)
        .filter(|group| !group.closed)
        .map(|group| group.open)
    else {
        return s.to_string();
    };

    let before = s.get(..start).unwrap_or("").trim_end();
    let content = s.get(start + 1..).unwrap_or("").trim_start();
    if before.is_empty() {
        if starts_with_measurement_fragment(content) {
            return content.to_string();
        }
        return s.to_string();
    }

    if let Some((measurement, item)) = split_malformed_parenthetical_package_item(content) {
        return join_segments(before, &join_segments(&measurement, &item));
    }

    if let Some((measurement, item)) = split_malformed_parenthetical_measurement_item(content) {
        return join_segments(&join_segments(before, &item), &format!("({})", measurement));
    }

    s.to_string()
}

fn is_bare_parenthetical_package_size(s: &str) -> bool {
    let (_, after_qualifier) = strip_leading_measurement_qualifier(s);
    measure::read_hyphenated_size(after_qualifier).is_some_and(|(_, unit, rest)| {
        rest.is_empty() && WEIGHT_UNITS_FOR_COMPOUND.contains(&unit.to_lowercase().as_str())
    })
}

fn is_standalone_parenthetical_package_fragment(s: &str) -> bool {
    let trimmed = s.trim();
    let Some(group) = tokens::first_group(trimmed).filter(|group| group.open == 0 && group.closed)
    else {
        return false;
    };
    let content = group.content(trimmed).trim();
    let after = trimmed
        .get(group.end(trimmed)..)
        .unwrap_or("")
        .trim()
        .trim_end_matches(['.', ',', ';'])
        .trim()
        .to_lowercase();

    if after.is_empty() || !CONTAINERS.iter().any(|container| after == *container) {
        return false;
    }

    !parse_parenthetical_measurement_details(content, "")
        .measurements
        .is_empty()
        || is_bare_parenthetical_package_size(content)
}

/// Words that size the alternative in "2 large or 3 smaller leeks" without
/// being a size unit on their own ("thin carrots" alone names a food). No
/// "little" or "big", which start food names ("Little Gem lettuces").
const ALTERNATIVE_SIZE_WORDS: &[&str] = &[
    "smaller", "larger", "thin", "thick", "slim", "skinny", "regular", "average",
];

/// "smaller leeks" -> ("smaller", "leeks"), "medium-sized lemons" ->
/// ("medium", "lemons"), "small-medium apples" -> ("small-medium", "apples").
/// Never leaves the item empty.
fn split_alternative_size(s: &str) -> Option<(String, String)> {
    let trimmed = s.trim_start();
    let (word, rest) = trimmed.split_once(char::is_whitespace)?;
    if !rest.trim_start().starts_with(char::is_alphabetic) {
        return None;
    }
    let lower = word.to_lowercase();
    let base = lower.strip_suffix("-sized").unwrap_or(&lower);
    let is_size = ALTERNATIVE_SIZE_WORDS.contains(&base) || base.split('-').all(is_size_unit);
    is_size.then(|| (normalize_unit(base), rest.trim_start().to_string()))
}

/// Leading words that say how a food is packed or how good it is, never
/// which food it is. Longer phrases first.
const LEADING_ITEM_QUALIFIERS: &[&str] = &[
    "firmly packed",
    "lightly packed",
    "loosely packed",
    "tightly packed",
    "well packed",
    "packed",
    "good quality",
    "good-quality",
    "high quality",
    "high-quality",
    "best quality",
    "best-quality",
    "top quality",
    "top-quality",
];

/// "packed dark-brown sugar" -> ("packed", "dark-brown sugar"). Never leaves
/// the item empty.
fn split_leading_item_qualifier(item: &str) -> Option<(&str, &str)> {
    let trimmed = item.trim_start();
    LEADING_ITEM_QUALIFIERS.iter().find_map(|qualifier| {
        let rest = trimmed
            .get(..qualifier.len())
            .filter(|prefix| prefix.eq_ignore_ascii_case(qualifier))
            .and_then(|_| trimmed.get(qualifier.len()..))?;
        let rest_trimmed = rest.trim_start();
        (rest.starts_with(char::is_whitespace) && rest_trimmed.starts_with(char::is_alphanumeric))
            .then(|| {
                (
                    trimmed.get(..qualifier.len()).unwrap_or(qualifier),
                    rest_trimmed,
                )
            })
    })
}

/// A size left at the front of the item, with the piece or container it sizes:
/// "8-ounce block", "2\" piece", "1-inch thick", "5-pound", "14-ounce each
/// cans", "5- to 5 1/2-ounce log", "3.4 oz".
static LEADING_SIZE_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    let nouns = [measure::PACKAGE_WORDS, measure::SIZED_PIECES]
        .concat()
        .iter()
        .map(|noun| format!("{noun}(?:e?s)?"))
        .collect::<Vec<_>>()
        .join("|");
    regex::Regex::new(&format!(
        r#"(?i)^(?P<size>\d+(?:\s+\d+/\d+|/\d+|\.\d+)?(?:\s*(?:-\s*to|-|to)\s*\d+(?:\s+\d+/\d+|/\d+|\.\d+)?)?(?:[-\s](?:inch(?:es)?\b|in\.)|["”]|(?:-|\s+)(?:ounces?|oz|pounds?|lbs?|grams?|g|kg|cm)\b\.?)(?:[-\s](?:thick|long|wide)\b)?(?:\s+each\b)?(?:\s+(?:{nouns})\b,?)?)\s+(?:of\s+)?(?P<rest>\p{{L}}.*)$"#
    ))
    .expect("Invalid leading size regex")
});

/// "8-ounce block halloumi" -> ("8-ounce block", "halloumi"): a size the
/// measurement reader left in front of the food. The item keeps it, since a
/// shopping list shows only the item and amount; the catalog matches the food
/// without it.
pub(crate) fn split_leading_size(item: &str) -> Option<(&str, &str)> {
    let caps = LEADING_SIZE_REGEX.captures(item.trim_start())?;
    let rest = caps.name("rest")?.as_str();
    // "12-inch round or 9x13 pizza doughs", "1-inch slices from a loaf": the
    // size doesn't lead a food.
    let first = rest.split_whitespace().next()?.to_lowercase();
    if ["or", "and", "from", "to"].contains(&first.as_str()) {
        return None;
    }
    Some((caps.name("size")?.as_str().trim_end_matches(','), rest))
}

/// After a bare amount, the rest of a list of alternative amounts: more bare
/// amounts after commas, then "or" and the last amount with the unit or food
/// they share (", 1/4 or 1/2 teaspoon yeast", ", 3, or 4 eggs"). Returns the
/// middle amounts and the read of the last one.
fn alternative_amount_list(rest: &str) -> Option<(Vec<String>, measure::Read)> {
    let segments = Segments::new(rest);
    let splits = segments.splits();
    if !segments.head().is_empty() || splits.first()?.sep != Sep::Comma {
        return None;
    }
    let mut amounts = Vec::new();
    for split in splits {
        match split.sep {
            // ", or": the comma before the last amount.
            Sep::Comma if split.segment.is_empty() => {}
            Sep::Comma => {
                let read = measure::read_measurement(split.segment);
                if read.modifier.is_some() || read.unit.is_some() || !read.rest.is_empty() {
                    return None;
                }
                amounts.push(read.amount?);
            }
            Sep::Or => {
                let read = measure::read_measurement(split.after);
                let names_food = read.unit.is_some() || read.rest.starts_with(char::is_alphabetic);
                return (!amounts.is_empty() && read.amount.is_some() && names_food)
                    .then_some((amounts, read));
            }
            _ => return None,
        }
    }
    None
}

/// "two US sticks unsalted butter" -> (2 stick, "unsalted butter").
fn split_stick_count(s: &str) -> Option<(Measurement, String)> {
    let normalized = normalize_word_numbers(s.trim_start());
    let (amount, after_amount) = extract_amount(&normalized);
    let amount = amount?;
    let after_us = after_amount.trim_start();
    let after_us = ["us ", "u.s. "]
        .iter()
        .find_map(|marker| {
            after_us
                .get(..marker.len())
                .filter(|prefix| prefix.eq_ignore_ascii_case(marker))
                .and_then(|_| after_us.get(marker.len()..))
        })
        .unwrap_or(after_us);
    let (unit, after_unit) = extract_unit(after_us);
    if normalize_unit(&unit?) != "stick" || after_unit.trim().is_empty() {
        return None;
    }
    Some((
        Measurement {
            amount: Some(amount),
            unit: Some("stick".to_string()),
        },
        after_unit,
    ))
}

/// Parse a single ingredient line into structured data.
///
/// This does best-effort parsing - if we can't parse something meaningful,
/// we return the raw text as the item with empty measurements.
/// Last words that make a Title Case name a food rather than a brand, so a
/// name ending in one keeps its own food before "or" ("Chicken Stock").
const TITLE_CASE_FOOD_NOUNS: &[&str] = &[
    "stock",
    "broth",
    "bouillon",
    "sauce",
    "salt",
    "sugar",
    "flour",
    "oil",
    "butter",
    "cheese",
    "cream",
    "milk",
    "water",
    "juice",
    "powder",
    "paste",
    "vinegar",
    "wine",
    "beer",
    "syrup",
    "honey",
    "rice",
    "beans",
    "pasta",
    "noodles",
    "bread",
    "chocolate",
    "seasoning",
    "mustard",
    "ketchup",
    "mayonnaise",
    "yogurt",
    "oats",
    "potatoes",
    "onions",
    "peppers",
    "tomatoes",
];

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

    // "Juice of 1 lemon": a line naming two parts ("zest and juice of ...")
    // only splits in parse_ingredients.
    if let Some(mut parts) = parse_citrus_part_line(raw).filter(|parts| parts.len() == 1) {
        return parts.remove(0);
    }

    // Decode HTML entities and normalize unicode before processing
    let decoded = decode_html_entities(raw);
    let normalized = normalize_unicode(&decoded);
    let normalized = strip_leading_list_marker(&normalized);
    let normalized = normalize_digit_letter_spacing(&normalized);
    let mut remaining = normalize_word_numbers(&normalized);
    let mut measurements = Vec::new();
    let mut note;

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

    // "Egg wash: 1 large egg beaten with 1 tablespoon milk": a short label
    // before a single measured ingredient goes to the note.
    if let Some((label, rest)) = split_label_before_amount(&remaining) {
        let mut parsed = parse_ingredient(rest);
        parsed.note = Some(match parsed.note {
            Some(existing) => format!("{label}, {existing}"),
            None => label.to_string(),
        });
        if optional_prefix {
            parsed.note = parsed.note.map(|n| format!("optional, {n}"));
        }
        parsed.raw = Some(raw.to_string());
        return parsed;
    }

    // Issue 5: Normalize double-wrapped parentheticals to single
    // e.g., "((about 4 cloves))" -> "(about 4 cloves)"
    remaining = unwrap_redundant_parentheses(&remaining);

    remaining = repair_unclosed_parenthetical_measurement_fragment(&remaining);

    remaining = strip_placeholder_parentheticals(&remaining);

    // Unwrap leading parentheticals that contain quantities
    // e.g., "(half stick) butter" -> "1/2 stick butter"
    // But NOT "(optional) 1/4 cup" which should keep the paren structure
    if let Some(group) =
        tokens::first_group(&remaining).filter(|group| group.open == 0 && group.close.is_some())
    {
        let paren_content = group.content(&remaining).trim();

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
            let after_paren = remaining.get(group.end(&remaining)..).unwrap_or("").trim();
            remaining = if after_paren.is_empty() {
                normalized_content
            } else {
                format!("{} {}", normalized_content, after_paren)
            };
        }
    }

    // Step 1: Extract any parenthetical content (measurements or prep notes)
    // e.g., "1 stick (113g) butter" -> extract "(113g)" as alt measurement
    // e.g., "1/2 cup butter (softened)" -> extract "(softened)" as note
    let mut alt_measurements = Vec::new();
    let mut deferred_parenthetical_notes = Vec::new();
    while let Some(group) = tokens::first_group(&remaining).filter(|group| group.close.is_some()) {
        // An unclosed doubled "(" ("((2 cups) flour") leaves one "(" inside.
        let paren_content = group.content(&remaining).trim_start_matches('(').trim();

        let raw_before_parenthetical = remaining.get(..group.open).unwrap_or("");
        let (follows_comma, follows_or) = parenthetical_branch_context(raw_before_parenthetical);
        let before_parenthetical = raw_before_parenthetical
            .trim_end()
            .trim_end_matches(',')
            .trim_end();
        let after_parenthetical = remaining
            .get(group.end(&remaining)..)
            .unwrap_or("")
            .trim_start()
            .trim_start_matches(')')
            .trim_start();

        // "1 (2-inch) piece ginger": the size belongs to the piece unit.
        if is_inch_size_before_piece(paren_content, after_parenthetical) {
            remaining = join_segments(
                &join_segments(before_parenthetical, paren_content),
                after_parenthetical,
            );
            continue;
        }

        let outside_text = join_segments(before_parenthetical, after_parenthetical);
        if let Some((item_segment, note_segment)) = split_parenthetical_item_identity(paren_content)
        {
            if outside_lacks_item_identity(&outside_text) {
                if let Some(note_segment) = note_segment {
                    push_deferred_parenthetical_note(
                        &mut deferred_parenthetical_notes,
                        &note_segment,
                        follows_comma,
                        follows_or,
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
        // "(about 1 cup fresh cilantro)" is a measurement restating the item,
        // even though "fresh" reads as prep.
        let skip_prep_note = (has_nested_parens && starts_with_digit)
            || try_parse_measurement_before_filler(paren_content, &outside_text).is_some();
        if is_prep_note(paren_content) && !skip_prep_note {
            // Strip leading comma (e.g., from raw like "tomato (, sliced)")
            let trimmed_content = paren_content
                .trim()
                .trim_start_matches(',')
                .trim()
                .to_string();
            // "(sliced very thin, about 1/4 cup worth)": a measurement part is
            // still an alternative, and the whole parenthetical stays the note.
            alt_measurements.extend(prep_note_parenthetical_measurements(
                paren_content,
                &outside_text,
            ));
            push_deferred_parenthetical_note(
                &mut deferred_parenthetical_notes,
                &trimmed_content,
                follows_comma,
                follows_or,
            );
            // Remove the parenthetical from remaining
            // Also strip trailing comma before the parenthetical (e.g., "onion, (diced)")
            remaining = join_segments(before_parenthetical, after_parenthetical);
            continue;
        }

        // Try to parse the parenthetical content as one or more measurements
        // Split by semicolons or commas to handle "8 ounces; 227 g each"
        let parsed_measurements =
            parse_parenthetical_measurement_details(paren_content, &outside_text);

        if !parsed_measurements.measurements.is_empty() {
            alt_measurements.extend(parsed_measurements.measurements);
            if parsed_measurements.has_non_measurement_content {
                let trimmed_content = paren_content
                    .trim()
                    .trim_start_matches(',')
                    .trim()
                    .to_string();
                push_deferred_parenthetical_note(
                    &mut deferred_parenthetical_notes,
                    &trimmed_content,
                    follows_comma,
                    follows_or,
                );
            }
            // Remove the parenthetical from remaining, preserving space
            // Also strip trailing comma before the parenthetical
            remaining = join_segments(before_parenthetical, after_parenthetical);
        } else {
            // Preserve any non-measurement parenthetical as ingredient-relevant note
            // instead of dropping it on the floor.
            let trimmed_content = bare_parenthetical_size(paren_content).unwrap_or_else(|| {
                paren_content
                    .trim()
                    .trim_start_matches(',')
                    .trim()
                    .to_string()
            });
            push_deferred_parenthetical_note(
                &mut deferred_parenthetical_notes,
                &trimmed_content,
                follows_comma,
                follows_or,
            );
            remaining = join_segments(before_parenthetical, after_parenthetical);
        }
    }

    // Step 2.9: "About 8 cups peanut oil": an approximation before the amount
    // goes to the note so the amount still parses.
    let mut approximation = None;
    {
        let (qualifier, after_qualifier) = strip_leading_measurement_qualifier(&remaining);
        if let Some(qualifier) = qualifier {
            if extract_amount(after_qualifier).0.is_some() {
                approximation = Some(qualifier.to_string());
                remaining = after_qualifier.to_string();
            }
        }
    }

    // Step 3: Read the measurement: modifiers, amount, unit phrase ("15
    // ounce can", "small handful", "2 x 400g cans") and "each".
    let read = measure::read_measurement(&remaining);
    let mut primary_amount = read.amount.clone();
    let mut primary_unit = read.modified_unit();
    remaining = read.rest.to_string();

    // Step 3.5: "1/8, 1/4 or 1/2 teaspoon yeast", "2, 3 or 4 eggs": a list
    // of bare amounts ending in an "or" alternative shares its unit.
    if primary_amount.is_some() && read.unit.is_none() {
        if let Some((amounts, last)) = alternative_amount_list(&remaining) {
            let shared_unit = match (&read.modifier, &last.unit) {
                (Some(m), Some(u)) => Some(format!("{m} {u}")),
                (Some(m), None) => Some(m.clone()),
                (None, u) => u.clone(),
            };
            let last_unit = if last.modifier.is_some() {
                last.modified_unit()
            } else {
                shared_unit.clone()
            };
            primary_unit = shared_unit.clone();
            alt_measurements.extend(amounts.into_iter().map(|amount| Measurement {
                amount: Some(amount),
                unit: shared_unit.clone(),
            }));
            alt_measurements.push(Measurement {
                amount: last.amount,
                unit: last_unit,
            });
            remaining = last.rest;
        }
    }

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

    // Step 4.4b: Handle "plus [amount] [unit]" compound quantities, including the
    // "+" spelling and chains like "3/4 cup plus 2 Tbsp. plus 1 tsp."
    // e.g., "1/2 cup plus 2 tablespoons flour" -> amount="1/2 cup plus 2 tablespoons", unit=null
    // This keeps the compound quantity together as a single amount rather than splitting into note
    loop {
        let segments = Segments::new(&remaining);
        let Some(plus) = segments.leading().filter(|s| s.sep == Sep::Plus) else {
            break;
        };
        let (after_plus, is_plus_sign) = (plus.after, plus.sign);

        // Try to parse a measurement from what follows "plus"
        let plus_measurement = parse_plus_continuation_measurement(after_plus);

        // Only combine if we got BOTH amount AND unit after "plus"
        if let Some((p_amt, p_unit, after_plus_unit, plus_prefix)) = plus_measurement {
            match (&primary_amount, &primary_unit) {
                // Combine into a single compound amount: "1/2 cup plus 2 tablespoons"
                (Some(amt), Some(unit)) => {
                    primary_amount = Some(format!(
                        "{} plus {}",
                        format_measurement_segment(amt, unit),
                        format_measurement_segment_with_prefix(
                            &p_amt,
                            &p_unit,
                            plus_prefix.as_deref()
                        )
                    ));
                    primary_unit = None; // Unit is now embedded in the compound amount
                    remaining = after_plus_unit;
                    continue;
                }
                // The amount is already compound: append the next segment
                (Some(amt), None) if amt.contains(" plus ") => {
                    primary_amount = Some(format!(
                        "{} plus {}",
                        amt,
                        format_measurement_segment_with_prefix(
                            &p_amt,
                            &p_unit,
                            plus_prefix.as_deref()
                        )
                    ));
                    remaining = after_plus_unit;
                    continue;
                }
                _ => {}
            }
        }

        if is_plus_sign && primary_amount.is_some() {
            // A stray "+" after the measurement (e.g., "1/4 cup + Oil") is a
            // formatting artifact; drop it and keep the rest as item text.
            remaining = after_plus.to_string();
        }
        break;
    }

    // Step 4.5: Handle " or " alternatives in remaining text
    // e.g., remaining = " or 3 heaping cups frozen pineapple"
    // Only split if what follows "or" is a valid measurement (has amount AND unit)
    // This avoids false positives like "vanilla or chocolate ice cream"
    if let Some(after_or) = Segments::new(&remaining).leading_after(Sep::Or) {
        // "or approximately 4 sausages": the approximation is dropped, as in
        // a parenthetical "(about 4 medium)".
        let (_, after_or) = strip_leading_measurement_qualifier(after_or);

        // "or 3 heaping cups frozen pineapple", "or 1/4 of a large onion"
        let read = measure::read_measurement(after_or);
        let after_or_unit = read.rest.clone();

        // "14 ounces or 4 plain pork sausages": right after a full
        // measurement, a bare count before the food is an alternative too.
        let bare_count = read.unit.is_none()
            && primary_amount.is_some()
            && primary_unit.is_some()
            && strip_measurement_modifier(after_or)
                .1
                .starts_with(|c: char| c.is_ascii_digit())
            && after_or_unit.starts_with(char::is_alphabetic)
            // "or 1 packet instant yeast" counts packets, not the food.
            && !after_or_unit
                .split_whitespace()
                .next()
                .is_some_and(is_package_noun);

        // Only treat as alternative if we got BOTH amount AND unit
        if read.amount.is_some() && (read.unit.is_some() || bare_count) {
            let mut or_unit = read.unit.as_ref().and(read.modified_unit());
            let mut after_or_unit = after_or_unit;
            // "2 large or 3 smaller leeks": after a sized count, the
            // alternative count is sized too.
            if bare_count && primary_unit.as_deref().is_some_and(is_size_unit) {
                if let Some((size, after_size)) = split_alternative_size(&after_or_unit) {
                    or_unit = Some(size);
                    after_or_unit = after_size;
                }
            }

            alt_measurements.push(Measurement {
                amount: read.amount,
                unit: or_unit,
            });

            remaining = after_or_unit;
        }
    }

    // Step 4.55: "1/2 lb. two US sticks unsalted butter": a count of sticks
    // after the measurement restates it.
    if primary_amount.is_some() && primary_unit.is_some() {
        if let Some((stick_measurement, after_sticks)) = split_stick_count(&remaining) {
            alt_measurements.push(stick_measurement);
            remaining = after_sticks;
        }
    }

    // Step 4.6: Handle " / " alternatives in remaining text
    // e.g., remaining = " / 100g celery root" (after parsing "3.5 ounces")
    // This handles metric/imperial alternatives like "3.5 oz / 100g"
    // Loop to handle multiple: "3/4 cup / 4 oz / 115g toasted sunflower seeds"
    loop {
        let segments = Segments::new(&remaining);
        let Some(after_slash) = segments.leading_after(Sep::Slash) else {
            break;
        };

        // Only treat as alternative if we got BOTH amount AND unit
        let read = measure::read_measurement(after_slash);
        if read.amount.is_some() && read.unit.is_some() {
            alt_measurements.push(Measurement {
                unit: read.modified_unit(),
                amount: read.amount,
            });

            remaining = read.rest;
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
        // Check for slash-separated alternative (e.g., "/8 oz." after "226g")
        // Only do this AFTER we've found at least one attached metric, to avoid
        // false positives like "1/2cup" being parsed as "1" then "/2 cup"
        if found_attached_metric {
            if let Some(after_slash) = Segments::new(&remaining).leading_after(Sep::Slash) {
                let read = measure::read_measurement(after_slash);
                if read.amount.is_some() && read.unit.is_some() {
                    alt_measurements.push(Measurement {
                        unit: read.modified_unit(),
                        amount: read.amount,
                    });
                    remaining = read.rest;
                    continue;
                }
            }
        }

        // Check for attached metric (e.g., "65g" at start of remaining)
        if let Some((attached_measurement, after_attached)) =
            measure::read_attached_metric(&remaining)
        {
            alt_measurements.push(attached_measurement);
            remaining = after_attached;
            found_attached_metric = true;
        } else {
            break;
        }
    }

    // Step 5: Classify what's left after the measurement into the item and
    // its note parts ("plus more", prep, guidance, "or" alternatives).
    let classified = classify::classify_item(
        &remaining,
        &mut primary_amount,
        primary_unit.as_deref(),
        &mut deferred_parenthetical_notes,
    );
    remaining = classified.item;
    note = classified.note;

    prepend_deferred_parenthetical_notes(&mut note, &deferred_parenthetical_notes);

    // Step 5.9: "packed dark-brown sugar", "good quality dark chocolate": how
    // the food is packed or how good it is goes to the note. It stays off the
    // unit so "1 cup" still converts by volume, and goes last so a package
    // weight ("28-ounce") still leads the note.
    if let Some((qualifier, item_part)) = split_leading_item_qualifier(&remaining) {
        note = Some(match note {
            Some(existing) => format!("{existing}, {qualifier}"),
            None => qualifier.to_string(),
        });
        remaining = item_part.to_string();
    }

    // Step 6: Build measurements list. "1 large or 2 small stalks celery":
    // the bare size shares the alternative's sized measure ("large stalk").
    if let Some(size) = primary_unit.as_deref().filter(|unit| is_size_unit(unit)) {
        let shared = alt_measurements.iter().find_map(|m| {
            let (alt_size, measure) = m.unit.as_deref()?.split_once(' ')?;
            is_size_unit(alt_size).then(|| measure.to_string())
        });
        if let Some(measure) = shared {
            primary_unit = Some(format!("{size} {measure}"));
        }
    }
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

    if let Some(approximation) = approximation {
        note = Some(match note {
            Some(n) => format!("{approximation}, {n}"),
            None => approximation,
        });
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

    // "1/2 envelope (about 1 1/2 teaspoons unflavored powdered gelatin)": the
    // count took everything outside the parentheses, so the food is the one
    // the note measures. In "1 cup, plus 2 tablespoons walnuts" the addition
    // measures the same food.
    let item = if item.is_empty() {
        note.as_deref()
            .filter(|note| !note.to_lowercase().starts_with("or "))
            .map(|note| {
                note.get(..5)
                    .filter(|plus| plus.eq_ignore_ascii_case("plus "))
                    .and_then(|_| note.get(5..))
                    .unwrap_or(note)
            })
            .and_then(|note| reparse_item(note).filter(|parsed| parsed.item != note))
            .filter(|parsed| !parsed.measurements.is_empty())
            .map(|parsed| parsed.item)
            .filter(|food| !is_only_prep_words(food))
            .unwrap_or_else(|| raw.to_string())
    } else {
        item
    };

    ParsedIngredient {
        item,
        measurements,
        note,
        raw: Some(raw.to_string()),
        section: None,
    }
}

/// An item text read as an ingredient line again, when the parse doesn't cut
/// into the name. A stored item can start with a number that is part of it
/// ("5- to 6-inch chiles", "85% lean ground beef", "1/4-inch-thick slices"),
/// so a parse only counts when what's left starts with a word.
pub fn reparse_item(item: &str) -> Option<ParsedIngredient> {
    let parsed = parse_ingredient(item);
    let lower = parsed.item.trim().to_lowercase();
    let starts_with_word = lower.chars().next().is_some_and(char::is_alphabetic);
    (starts_with_word && !lower.starts_with("percent ")).then_some(parsed)
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

/// Scan a line's parenthesis balance: the depth left open at the end, and
/// whether the balance ever dipped below zero (a ')' with no matching '(').
fn scan_paren_balance(s: &str) -> (i32, bool) {
    let mut depth: i32 = 0;
    let mut dipped_negative = false;
    for c in s.chars() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    dipped_negative = true;
                    depth = 0;
                }
            }
            _ => {}
        }
    }
    (depth, dipped_negative)
}

/// True if the line opens a parenthesis it never closes.
fn has_unclosed_paren(s: &str) -> bool {
    scan_paren_balance(s).0 > 0
}

/// True if the line closes a parenthesis opened on a previous line.
fn closes_open_paren(s: &str) -> bool {
    scan_paren_balance(s).1
}

pub fn next_peer_section_anchor(section_name: &str) -> Option<String> {
    (section_name == "Crumbs").then(|| section_name.to_string())
}

pub fn detect_peer_section_header(raw: &str, peer_section_anchor: Option<&str>) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.contains(':')
        || trimmed.chars().any(|c| c.is_ascii_digit())
        || !trimmed.chars().any(|c| c.is_alphabetic())
    {
        return None;
    }

    let lower = trimmed.to_lowercase();
    let expected = match peer_section_anchor {
        Some("Crumbs") => "caramel",
        Some("Caramel") => "whipped cream",
        Some("Whipped Cream") => "peaches",
        _ => return None,
    };
    if lower != expected {
        return None;
    }

    Some(normalize_section_name(trimmed))
}

/// Section header for a line of a Paprika ingredients field: whatever
/// `detect_section_header` accepts, named the same way, or else any other line
/// ending in a colon, named as written. Paprika text is typed by the user (or
/// written by Ramekin's export, which puts a `Section:` line before each
/// section), so a colon there marks a header even when scraped text's guards
/// would reject it ("Part 1:").
pub fn detect_paprika_section_header(raw: &str) -> Option<String> {
    detect_section_header(raw).or_else(|| {
        raw.trim()
            .strip_suffix(':')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
    })
}

/// Parse multiple ingredient lines (separated by newlines).
/// Detects section headers (lines ending with colon, no measurements) and
/// applies the section name to subsequent ingredients.
/// Skips lines that should be ignored (scraper artifacts like "Gather Your Ingredients").
pub fn parse_ingredients(blob: &str) -> Vec<ParsedIngredient> {
    parse_ingredient_lines(blob, false)
}

/// Parse a Paprika ingredients field, which is plain text the user typed (or
/// Ramekin's own export wrote) rather than scraped HTML, so its layout can be
/// trusted more:
/// - Any line ending in a colon is a section header
///   (`detect_paprika_section_header`).
/// - A blank line after a section's ingredients ends that section, so the
///   ingredients below it have no section until the next header.
pub fn parse_paprika_ingredients(blob: &str) -> Vec<ParsedIngredient> {
    parse_ingredient_lines(blob, true)
}

fn parse_ingredient_lines(blob: &str, paprika: bool) -> Vec<ParsedIngredient> {
    let mut current_section: Option<String> = None;
    let mut section_has_ingredients = false;
    let mut peer_section_anchor: Option<String> = None;
    let mut results = Vec::new();

    let mut lines = blob.lines().peekable();
    while let Some(line) = lines.next() {
        // Re-join hard-wrapped lines: when a parenthetical opens on one line and
        // a following line closes it, the source wrapped a single ingredient.
        let mut line = line.to_string();
        while has_unclosed_paren(&line) && lines.peek().is_some_and(|next| closes_open_paren(next))
        {
            line.push(' ');
            line.push_str(lines.next().expect("peeked line exists").trim());
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            if paprika && section_has_ingredients {
                current_section = None;
                peer_section_anchor = None;
                section_has_ingredients = false;
            }
            continue;
        }
        // The raw text stays the line as written, list marker included.
        let original = trimmed.to_string();
        let normalized = strip_leading_list_marker(trimmed);
        let trimmed = normalized.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Skip lines that should be ignored (scraper artifacts)
        if should_ignore_line(trimmed) {
            continue;
        }

        if is_standalone_parenthetical_package_fragment(trimmed) {
            continue;
        }

        // Check if this line is a section header
        let section_header = if paprika {
            detect_paprika_section_header(trimmed)
        } else {
            detect_section_header(trimmed)
        };
        if let Some(section_name) = section_header {
            peer_section_anchor = next_peer_section_anchor(&section_name);
            current_section = Some(section_name);
            section_has_ingredients = false;
            continue; // Don't emit the header as an ingredient
        }
        if let Some(section_name) =
            detect_peer_section_header(trimmed, peer_section_anchor.as_deref())
        {
            peer_section_anchor = Some(section_name.clone());
            current_section = Some(section_name);
            section_has_ingredients = false;
            continue;
        }
        section_has_ingredients = current_section.is_some();

        // "Zest and juice of 1 lemon" names two ingredients; each keeps the
        // original line as its raw text.
        if let Some(parts) = parse_citrus_part_line(trimmed) {
            for mut ingredient in parts {
                ingredient.raw = Some(original.clone());
                ingredient.section = current_section.clone();
                results.push(ingredient);
            }
            continue;
        }

        // Split bare compound lists ("salt, pepper and cumin") into one
        // ingredient per item. Each split ingredient keeps the original line
        // as its raw text, mirroring the "each" expansion below.
        if let Some(parts) = split_bare_compound_line(trimmed) {
            for part in parts {
                let mut ingredient = parse_ingredient(&part);
                ingredient.raw = Some(original.clone());
                ingredient.section = current_section.clone();
                results.push(ingredient);
            }
            continue;
        }

        // Parse the ingredient and apply current section
        let mut ingredient = parse_ingredient(trimmed);
        ingredient.raw = Some(original);
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
    fn test_parse_paprika_ingredients_trusts_colon_headers() {
        let blob = "Part 1:\n1 cup flour\nfor the crust:\nsalt\nPart 1:\n1 egg";
        let result = parse_paprika_ingredients(blob);

        let sections: Vec<_> = result
            .iter()
            .map(|i| (i.section.as_deref(), i.item.as_str()))
            .collect();
        assert_eq!(
            sections,
            [
                (Some("Part 1"), "flour"),
                // Headers scraped text also accepts are named the same way.
                (Some("For the Crust"), "salt"),
                (Some("Part 1"), "egg"),
            ]
        );
        // Scraped text keeps its stricter header rules.
        assert_eq!(parse_ingredients(blob)[0].item, "Part 1:");
    }

    #[test]
    fn test_parse_paprika_ingredients_blank_line_ends_a_section() {
        let blob = "1 cup flour\nCrust:\n\n2 tbsp butter\n\n1 tsp salt\nFILLING\n6 apples\n\nsugar";
        let result = parse_paprika_ingredients(blob);

        let sections: Vec<_> = result
            .iter()
            .map(|i| (i.section.as_deref(), i.item.as_str()))
            .collect();
        assert_eq!(
            sections,
            [
                (None, "flour"),
                // A blank line right after a header doesn't end the section.
                (Some("Crust"), "butter"),
                (None, "salt"),
                (Some("Filling"), "apples"),
                (None, "sugar"),
            ]
        );
        // Scraped text keeps a section across blank lines.
        assert_eq!(parse_ingredients(blob)[2].section.as_deref(), Some("Crust"));
    }

    #[test]
    fn test_parse_ingredients_title_case_sections_without_colons() {
        let blob = "Lid\n2 cups flour\nFilling\n2 tablespoons olive oil";
        let result = parse_ingredients(blob);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].section, Some("Lid".to_string()));
        assert_eq!(result[0].item, "flour");
        assert_eq!(result[1].section, Some("Filling".to_string()));
        assert_eq!(result[1].item, "olive oil");
    }

    #[test]
    fn test_parse_ingredients_crumbs_peer_sections_without_colons() {
        let blob = [
            "Crumbs",
            "1 cup flour",
            "Caramel",
            "2/3 cup sugar",
            "Whipped cream",
            "1/2 cup heavy cream",
            "Peaches",
            "4 large peaches",
        ]
        .join("\n");
        let result = parse_ingredients(&blob);

        assert_eq!(result.len(), 4);
        assert_eq!(result[0].item, "flour");
        assert_eq!(result[0].section, Some("Crumbs".to_string()));
        assert_eq!(result[1].item, "sugar");
        assert_eq!(result[1].section, Some("Caramel".to_string()));
        assert_eq!(result[2].item, "heavy cream");
        assert_eq!(result[2].section, Some("Whipped Cream".to_string()));
        assert_eq!(result[3].item, "peaches");
        assert_eq!(result[3].section, Some("Peaches".to_string()));
    }

    #[test]
    fn test_parse_ingredients_peer_labels_stay_ingredients_without_crumbs_section() {
        let blob = "Peaches\nWhipped cream\nToppings\nWhipped cream";
        let result = parse_ingredients(blob);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].item, "Peaches");
        assert_eq!(result[0].section, None);
        assert_eq!(result[1].item, "Whipped cream");
        assert_eq!(result[1].section, None);
        assert_eq!(result[2].item, "Whipped cream");
        assert_eq!(result[2].section, Some("Toppings".to_string()));
    }

    #[test]
    fn test_parse_ingredients_peach_split_peers_do_not_follow_ordinary_caramel_section() {
        let blob = "Caramel:\nWhipped cream\npeaches\n1 cup sugar";
        let result = parse_ingredients(blob);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].item, "Whipped cream");
        assert_eq!(result[0].section, Some("Caramel".to_string()));
        assert_eq!(result[1].item, "peaches");
        assert_eq!(result[1].section, Some("Caramel".to_string()));
        assert_eq!(result[2].item, "sugar");
        assert_eq!(result[2].section, Some("Caramel".to_string()));
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
    fn test_parse_ingredient_repairs_unmatched_leading_measurement_paren() {
        let result = parse_ingredient("(16-ounce can chickpeas");

        assert_eq!(result.item, "chickpeas");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(
            result.measurements[0].unit,
            Some("16-ounce can".to_string())
        );
        assert_eq!(result.note, None);
    }

    #[test]
    fn test_parse_ingredient_repairs_unclosed_package_parenthetical_after_count() {
        let result = parse_ingredient("1 (16-ounce can chickpeas or 2 cups homemade chickpeas");

        assert_eq!(result.item, "chickpeas");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(
            result.measurements[0].unit,
            Some("16-ounce can".to_string())
        );
        assert_eq!(
            result.note,
            Some("or 2 cups homemade chickpeas".to_string())
        );
    }

    #[test]
    fn test_parse_ingredient_repairs_unclosed_parenthetical_measurement_note() {
        let result = parse_ingredient("1 cup (about 3 to 3 1/2 ouncesgrated Parmesan cheese");

        assert_eq!(result.item, "grated Parmesan cheese");
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
        assert_eq!(
            result.measurements[1].amount,
            Some("3 to 3 1/2".to_string())
        );
        assert_eq!(result.measurements[1].unit, Some("oz".to_string()));
        assert_eq!(result.note, Some("about 3 to 3 1/2 ounces".to_string()));
    }

    #[test]
    fn test_parse_ingredients_drops_standalone_parenthetical_package_fragment() {
        let blob = [
            "2 cups white beans, cooked and drained",
            "(440-gram) cans",
            "(16-ounce) cans",
            "(1-pound) packages",
        ]
        .join("\n");
        let result = parse_ingredients(&blob);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].item, "white beans");
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
    fn test_plus_sign_compound_measurement() {
        // "+" between two measurements works like "plus"
        let result = parse_ingredient("1 tablespoon + 1 1/4 teaspoons salt, divided");
        assert_eq!(result.item, "salt");
        assert_eq!(
            result.measurements[0].amount,
            Some("1 tablespoon plus 1 1/4 teaspoons".to_string())
        );
        assert_eq!(result.measurements[0].unit, None);
        assert_eq!(result.note.as_deref(), Some("divided"));
    }

    #[test]
    fn test_plus_sign_compound_measurement_no_space() {
        let result = parse_ingredient("1/3 cup +2 Tbsp warm water");
        assert_eq!(result.item, "warm water");
        assert_eq!(
            result.measurements[0].amount,
            Some("1/3 cup plus 2 tbsp".to_string())
        );
    }

    #[test]
    fn test_plus_sign_compound_measurement_with_modifier() {
        let result = parse_ingredient(
            "7 cups + about 6 Tbsp unbleached all-purpose flour (plus more to dust)",
        );
        assert_eq!(result.item, "unbleached all-purpose flour");
        assert_eq!(
            result.measurements[0].amount,
            Some("7 cups plus about 6 tbsp".to_string())
        );
        assert_eq!(result.measurements[0].unit, None);
        assert_eq!(result.note.as_deref(), Some("plus more to dust"));
    }

    #[test]
    fn test_parenthetical_identity_supplies_item_after_compound_measurement() {
        let result = parse_ingredient("1/3 cup + 2 Tbsp (100 mL white vinegar)");
        assert_eq!(result.item, "white vinegar");
        assert_eq!(
            result.measurements[0].amount,
            Some("1/3 cup plus 2 tbsp".to_string())
        );
        assert_eq!(result.measurements[0].unit, None);
        assert_eq!(result.note.as_deref(), Some("100 mL white vinegar"));
    }

    #[test]
    fn test_parenthetical_identity_supplies_item_after_modified_compound_measurement() {
        let result = parse_ingredient("1/3 cup + about 2 Tbsp (100 mL white vinegar)");
        assert_eq!(result.item, "white vinegar");
        assert_eq!(
            result.measurements[0].amount,
            Some("1/3 cup plus about 2 tbsp".to_string())
        );
        assert_eq!(result.measurements[0].unit, None);
        assert_eq!(result.note.as_deref(), Some("100 mL white vinegar"));
    }

    #[test]
    fn test_parenthetical_identity_supplies_item_after_word_plus_measurement() {
        let result = parse_ingredient("1/3 cup plus 2 Tbsp (100 mL white vinegar)");
        assert_eq!(result.item, "white vinegar");
        assert_eq!(
            result.measurements[0].amount,
            Some("1/3 cup plus 2 tbsp".to_string())
        );
        assert_eq!(result.measurements[0].unit, None);
        assert_eq!(result.note.as_deref(), Some("100 mL white vinegar"));
    }

    #[test]
    fn test_word_plus_compound_measurement_with_modifier() {
        let result = parse_ingredient("1 cup plus about 2 Tbsp flour");
        assert_eq!(result.item, "flour");
        assert_eq!(
            result.measurements[0].amount,
            Some("1 cup plus about 2 tbsp".to_string())
        );
        assert_eq!(result.measurements[0].unit, None);
    }

    #[test]
    fn test_word_plus_modified_measurement_after_item_is_note() {
        let result = parse_ingredient("2/3 cup powdered sugar plus about 2 tbsp for dusting");
        assert_eq!(result.item, "powdered sugar");
        assert_eq!(
            result.note.as_deref(),
            Some("plus about 2 tbsp for dusting")
        );
    }

    #[test]
    fn test_parenthetical_identity_supplies_item_after_word_plus_modified_measurement() {
        let result = parse_ingredient("1/3 cup plus about 2 Tbsp (100 mL white vinegar)");
        assert_eq!(result.item, "white vinegar");
        assert_eq!(
            result.measurements[0].amount,
            Some("1/3 cup plus about 2 tbsp".to_string())
        );
        assert_eq!(result.measurements[0].unit, None);
        assert_eq!(result.note.as_deref(), Some("100 mL white vinegar"));
    }

    #[test]
    fn test_uppercase_t_period_is_tablespoon() {
        let result = parse_ingredient("2 T. ice water");
        assert_eq!(result.item, "ice water");
        assert_eq!(result.measurements[0].amount, Some("2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("tbsp".to_string()));

        let result = parse_ingredient("2 t. kosher salt");
        assert_eq!(result.item, "kosher salt");
        assert_eq!(result.measurements[0].amount, Some("2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("tsp".to_string()));
    }

    #[test]
    fn test_chained_plus_compound_measurement() {
        // Chains of "plus" segments fold into a single compound amount
        let result = parse_ingredient("3/4 cup plus 2 Tbsp. plus 1 tsp. sugar, divided");
        assert_eq!(result.item, "sugar");
        assert_eq!(
            result.measurements[0].amount,
            Some("3/4 cup plus 2 tbsp plus 1 tsp".to_string())
        );
        assert_eq!(result.note.as_deref(), Some("divided"));
    }

    #[test]
    fn test_stray_plus_between_amount_and_unit() {
        // "3+ cups" / "1/4 + teaspoon" style: the "+" is junk between amount and unit
        let result = parse_ingredient("3+ cups sugar, mixed types");
        assert_eq!(result.item, "sugar");
        assert_eq!(result.measurements[0].amount, Some("3".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));

        let result = parse_ingredient("1/4 + teaspoon fine grain sea salt");
        assert_eq!(result.item, "fine grain sea salt");
        assert_eq!(result.measurements[0].amount, Some("1/4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("tsp".to_string()));
    }

    #[test]
    fn test_plus_sign_mixed_number_amount() {
        // "1 + 1/2 teaspoons" is a mixed number written with an explicit "+"
        let result = parse_ingredient("1 + 1/2 teaspoons potato starch");
        assert_eq!(result.item, "potato starch");
        assert_eq!(result.measurements[0].amount, Some("1 1/2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("tsp".to_string()));

        let result = parse_ingredient("2 + ¼ cups light brown sugar");
        assert_eq!(result.item, "light brown sugar");
        assert_eq!(result.measurements[0].amount, Some("2 1/4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_plus_sign_fraction_sum_amount() {
        // Two fractions sharing one unit fold into a compound amount
        let result = parse_ingredient("1/4 + 1/8 teaspoon fine sea salt");
        assert_eq!(result.item, "fine sea salt");
        assert_eq!(
            result.measurements[0].amount,
            Some("1/4 plus 1/8".to_string())
        );
        assert_eq!(result.measurements[0].unit, Some("tsp".to_string()));
    }

    #[test]
    fn test_stray_plus_before_item() {
        // A "+" after the measurement that is not followed by another measurement
        let result = parse_ingredient("1/4 teaspoon + fine-grain sea salt");
        assert_eq!(result.item, "fine-grain sea salt");
        assert_eq!(result.measurements[0].amount, Some("1/4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("tsp".to_string()));

        let result = parse_ingredient("1/4 cup + Oil");
        assert_eq!(result.item, "Oil");
        assert_eq!(result.measurements[0].amount, Some("1/4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_mid_item_plus_more_becomes_note() {
        let result = parse_ingredient("1 tablespoon olive oil + more for drizzling");
        assert_eq!(result.item, "olive oil");
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("tbsp".to_string()));
        assert_eq!(result.note.as_deref(), Some("plus more for drizzling"));
    }

    #[test]
    fn test_mid_item_plus_same_unit_amount_folds_into_measurement() {
        let result = parse_ingredient("3/4 tsp salt + 1/8 tsp");
        assert_eq!(result.item, "salt");
        assert_eq!(
            result.measurements[0].amount,
            Some("3/4 plus 1/8".to_string())
        );
        assert_eq!(result.measurements[0].unit, Some("tsp".to_string()));
    }

    #[test]
    fn test_parse_ingredients_drops_footnote_lines() {
        let blob = "1 cup flour\n*Substitute octopus with sausage (cheese, ham, and etc.)\n**if more milk is needed, add 1/4 cup milk at a time";
        let result = parse_ingredients(blob);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].item, "flour");
    }

    #[test]
    fn test_parse_ingredients_keeps_asterisk_bullet_lines() {
        // "* " is a list bullet, not a footnote marker
        let blob = "* 1 cup flour\n* 2 eggs";
        let result = parse_ingredients(blob);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].item, "flour");
        assert_eq!(result[1].item, "eggs");
    }

    #[test]
    fn test_parse_ingredients_merges_wrapped_parenthetical_lines() {
        // Hard-wrapped source line: the parenthetical opens on one line and
        // closes on the next, so the two lines are one ingredient.
        let blob =
            "2 envelopes unflavored powdered gelatin (about 1 Tbsp. plus 2\ntsp.)\n1 2/3 cups whole milk";
        let result = parse_ingredients(blob);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].item, "unflavored powdered gelatin");
        assert_eq!(result[1].item, "whole milk");

        let blob = "6 tablespoons (90 grams) sour cream or whole Greek yogurt (i.e., a strained\nyogurt)\n1 tablespoon (15 ml) white wine vinegar";
        let result = parse_ingredients(blob);
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].item, "white wine vinegar");
    }

    #[test]
    fn test_parse_ingredients_no_merge_when_paren_never_closed() {
        // An unclosed paren whose next line doesn't close it is just a typo in
        // the source; the lines are separate ingredients.
        let blob = "1 (16-ounce can chickpeas\n2 cups water";
        let result = parse_ingredients(blob);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_ingredients_splits_bare_compound_lines() {
        let blob = "For the rub:\nsalt, black pepper and smoked paprika\n2 lbs pork shoulder";
        let result = parse_ingredients(blob);

        assert_eq!(result.len(), 4);
        assert_eq!(result[0].item, "salt");
        assert_eq!(result[1].item, "black pepper");
        assert_eq!(result[2].item, "smoked paprika");
        for ingredient in &result[..3] {
            assert_eq!(ingredient.section, Some("For the Rub".to_string()));
            assert_eq!(
                ingredient.raw,
                Some("salt, black pepper and smoked paprika".to_string())
            );
            assert!(ingredient.measurements.is_empty());
        }
        assert_eq!(result[3].item, "pork shoulder");
    }

    #[test]
    fn test_parse_ingredients_keeps_noted_lines_intact() {
        // A qualifier tail means the comma is a note, not a list separator.
        let blob = "2 cups flour, sifted\nolive oil, for frying\nsalt and pepper";
        let result = parse_ingredients(blob);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].item, "flour");
        assert_eq!(result[2].item, "salt and pepper");
    }

    fn measurements(parsed: &ParsedIngredient) -> Vec<(Option<&str>, Option<&str>)> {
        parsed
            .measurements
            .iter()
            .map(|m| (m.amount.as_deref(), m.unit.as_deref()))
            .collect()
    }

    #[test]
    fn test_parse_stick_count_after_measurement() {
        let parsed = parse_ingredient("1/2 lb. two US sticks unsalted butter");
        assert_eq!(parsed.item, "unsalted butter");
        assert_eq!(
            measurements(&parsed),
            vec![(Some("1/2"), Some("lb")), (Some("2"), Some("stick"))]
        );

        assert_eq!(
            parse_ingredient("1/2 lb. two us sticks unsalted butter").item,
            "unsalted butter"
        );

        let parsed = parse_ingredient("3/4 cup 1 1/2 sticks/170g unsalted butter");
        assert_eq!(parsed.item, "unsalted butter");
        assert_eq!(
            measurements(&parsed),
            vec![
                (Some("3/4"), Some("cup")),
                (Some("1 1/2"), Some("stick")),
                (Some("170"), Some("g"))
            ]
        );
    }

    #[test]
    fn test_parse_sized_container_unit() {
        let parsed = parse_ingredient("3 6-ounce containers fresh raspberries");
        assert_eq!(parsed.item, "fresh raspberries");
        assert_eq!(
            measurements(&parsed),
            vec![(Some("3"), Some("6-ounce containers"))]
        );
    }

    #[test]
    fn test_parse_inch_sized_piece_unit() {
        for (raw, unit, item) in [
            ("1 3-4 inch chunk of ginger", "3-4 inch chunk", "ginger"),
            (
                "1 3-inch piece fresh ginger",
                "3-inch piece",
                "fresh ginger",
            ),
            ("A 3-inch piece of ginger", "3-inch piece", "ginger"),
            ("A 1 1/2-inch piece of ginger", "1 1/2-inch piece", "ginger"),
            (
                "Four 3-inch knobs fresh ginger",
                "3-inch knobs",
                "fresh ginger",
            ),
            ("1 1 1/2-inch piece ginger", "1 1/2-inch piece", "ginger"),
            (
                "1 (1 1/2-inch) piece fresh ginger",
                "1 1/2-inch piece",
                "fresh ginger",
            ),
            ("1 (2-inch) piece ginger", "2-inch piece", "ginger"),
            ("2 (2 inch) chunks of ginger", "2 inch chunks", "ginger"),
        ] {
            let parsed = parse_ingredient(raw);
            assert_eq!(parsed.item, item, "{raw}");
            assert_eq!(parsed.measurements[0].unit.as_deref(), Some(unit), "{raw}");
        }
    }

    #[test]
    fn test_parse_size_after_of_a() {
        let parsed = parse_ingredient("1/2 of a small or 1/4 of a large white onion");
        assert_eq!(parsed.item, "white onion");
        assert_eq!(
            measurements(&parsed),
            vec![(Some("1/2"), Some("small")), (Some("1/4"), Some("large"))]
        );
        assert_eq!(parse_ingredient("Half of a lemon").item, "lemon");
    }

    #[test]
    fn test_parse_bare_count_alternative_after_measurement() {
        let parsed =
            parse_ingredient("14 ounces (400 grams) or approximately 4 plain pork sausages");
        assert_eq!(parsed.item, "plain pork sausages");
        assert_eq!(
            measurements(&parsed),
            vec![
                (Some("14"), Some("oz")),
                (Some("400"), Some("g")),
                (Some("4"), None)
            ]
        );
        // A counted packet is an alternative measure of the food.
        let parsed = parse_ingredient("2 1/4 teaspoons or 1 packet instant yeast");
        assert_eq!(parsed.item, "instant yeast");
        assert_eq!(
            measurements(&parsed),
            vec![(Some("2 1/4"), Some("tsp")), (Some("1"), Some("packet"))]
        );
    }

    #[test]
    fn test_parse_sized_count_alternative() {
        for (raw, item, size) in [
            ("2 large or 3 smaller leeks", "leeks", "smaller"),
            ("1 large or 2 thin carrots", "carrots", "thin"),
            (
                "3 large or 4 regular cloves garlic",
                "cloves garlic",
                "regular",
            ),
            ("5 large or 6 medium-sized lemons", "lemons", "medium"),
            ("4 large or 5 small-medium apples", "apples", "small-medium"),
        ] {
            let parsed = parse_ingredient(raw);
            assert_eq!(parsed.item, item, "{raw}");
            assert_eq!(parsed.measurements[1].unit.as_deref(), Some(size), "{raw}");
        }
        // Without a sized primary, the word stays with the food.
        let parsed = parse_ingredient("14 ounces or 4 thin pork sausages");
        assert_eq!(parsed.item, "thin pork sausages");
        assert_eq!(parsed.measurements[1].unit, None);
        let parsed = parse_ingredient("1 large or 2 Little Gem lettuces");
        assert_eq!(parsed.item, "Little Gem lettuces");
    }

    #[test]
    fn test_parse_dash_spaced_on_one_side() {
        for (raw, amount, unit, item) in [
            ("2 -3   teaspoons    honey", "2-3", "tsp", "honey"),
            (
                "4- 5 cups all-purpose flour",
                "4-5",
                "cup",
                "all-purpose flour",
            ),
            ("1/4 -1/2 tsp salt", "1/4-1/2", "tsp", "salt"),
            (
                "2– 2 ½   cups of warm water",
                "2-2 1/2",
                "cup",
                "warm water",
            ),
            ("1- pound ground beef", "1", "lb", "ground beef"),
            ("2- cups flour", "2", "cup", "flour"),
            ("3- cloves garlic", "3", "clove", "garlic"),
            ("1- 2 cans black beans", "1-2", "can", "black beans"),
            // The size sizes the cans; it isn't the unit.
            ("1- 2 large cans tomatoes", "1-2", "large can", "tomatoes"),
            (
                "1- inch knob fresh ginger",
                "1",
                "1-inch knob",
                "fresh ginger",
            ),
            (
                "12- ounces frozen orange juice concentrate",
                "12",
                "oz",
                "frozen orange juice concentrate",
            ),
        ] {
            let parsed = parse_ingredient(raw);
            assert_eq!(parsed.item, item, "{raw}");
            assert_eq!(
                parsed.measurements[0].amount.as_deref(),
                Some(amount),
                "{raw}"
            );
            assert_eq!(parsed.measurements[0].unit.as_deref(), Some(unit), "{raw}");
        }
        // One sized package, not a range.
        for (raw, item) in [
            ("1- 15 Ounce Can Tomato Sauce", "Tomato Sauce"),
            ("1- 3 ounce package cream cheese", "cream cheese"),
        ] {
            let parsed = parse_ingredient(raw);
            assert_eq!(parsed.item, item, "{raw}");
            assert_eq!(parsed.measurements[0].amount.as_deref(), Some("1"), "{raw}");
        }
    }

    #[test]
    fn test_parse_title_case_food_before_or_keeps_its_name() {
        assert_eq!(
            parse_ingredient("2 cups Chicken Stock or 2 cups water").item,
            "Chicken Stock"
        );
        for (raw, item) in [
            (
                "1 tsp Diamond Crystal or 1/2 tsp Morton kosher salt",
                "Diamond Crystal kosher salt",
            ),
            (
                "1 tsp Diamond Crystal or 1/2 tsp kosher salt",
                "Diamond Crystal kosher salt",
            ),
            (
                "2 cups Better Than Bouillon or 2 cups water",
                "Better Than Bouillon",
            ),
        ] {
            assert_eq!(parse_ingredient(raw).item, item, "{raw}");
        }
    }

    #[test]
    fn test_parse_orange_before_count_alternative() {
        let parsed = parse_ingredient("1 large orange or 2 small ones, juiced");
        assert_eq!(parsed.item, "orange");
        assert_eq!(parsed.note.as_deref(), Some("or 2 small ones, juiced"));
        assert_eq!(
            parse_ingredient("3 red, orange, and yellow bell peppers").item,
            "red, orange, and yellow bell peppers"
        );
    }

    #[test]
    fn test_parse_packing_and_quality_words_go_to_note() {
        for (raw, item, note) in [
            (
                "2/3 cup packed dark-brown sugar",
                "dark-brown sugar",
                "packed",
            ),
            (
                "1 1/2 cups packed (285 grams) dark-brown sugar",
                "dark-brown sugar",
                "packed",
            ),
            (
                "1 cup firmly packed light brown sugar",
                "light brown sugar",
                "firmly packed",
            ),
            (
                "6 ounces good quality dark chocolate (70-75%), finely chopped",
                "dark chocolate",
                "70-75%, finely chopped, good quality",
            ),
            (
                "good quality kitchen shears",
                "kitchen shears",
                "good quality",
            ),
            ("1 pound high-quality butter", "butter", "high-quality"),
        ] {
            let parsed = parse_ingredient(raw);
            assert_eq!(parsed.item, item, "{raw}");
            assert_eq!(parsed.note.as_deref(), Some(note), "{raw}");
        }
        // The unit stays a plain volume so it still converts.
        assert_eq!(
            parse_ingredient("2/3 cup packed dark-brown sugar").measurements[0]
                .unit
                .as_deref(),
            Some("cup")
        );
        // A packed measure is still a modified unit.
        let parsed = parse_ingredient("1 packed cup brown sugar");
        assert_eq!(parsed.item, "brown sugar");
        assert_eq!(parsed.measurements[0].unit.as_deref(), Some("packed cup"));
    }

    #[test]
    fn test_parse_sized_loose_measure() {
        let parsed = parse_ingredient("Small handful picked fresh rosemary leaves, finely chopped");
        assert_eq!(parsed.item, "picked fresh rosemary leaves");
        assert_eq!(measurements(&parsed), vec![(None, Some("small handful"))]);

        assert_eq!(
            parse_ingredient("1 XL pinch saffron").measurements[0]
                .unit
                .as_deref(),
            Some("extra-large pinch")
        );

        let parsed = parse_ingredient("1 large bunch kale");
        assert_eq!(parsed.item, "kale");
        assert_eq!(
            measurements(&parsed),
            vec![(Some("1"), Some("large bunch"))]
        );

        assert_eq!(
            parse_ingredient("2 large pinches (of Spanish saffron threads)").item,
            "pinches"
        );

        let parsed = parse_ingredient("1/2 of a small head of garlic");
        assert_eq!(parsed.item, "garlic");
        assert_eq!(
            measurements(&parsed),
            vec![(Some("1/2"), Some("small head"))]
        );

        // A size before the food stays the unit.
        let parsed = parse_ingredient("1 large onion");
        assert_eq!(parsed.item, "onion");
        assert_eq!(measurements(&parsed), vec![(Some("1"), Some("large"))]);
    }
}
