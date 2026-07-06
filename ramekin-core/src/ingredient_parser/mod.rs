//! Ingredient parsing module.
//!
//! Parses raw ingredient strings (e.g., "2 cups flour, sifted") into structured data.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::metric_weights::parse_amount;
use crate::text::decode_html_entities;

mod amounts;
mod fractions;
mod item;
mod parentheticals;
mod units;

use amounts::*;
use item::*;
use parentheticals::*;
use units::*;

pub(in crate::ingredient_parser) use fractions::unicode_fraction_ascii;
pub(crate) use fractions::unicode_fraction_regex_class;
pub use item::{detect_section_header, should_ignore_line};

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
    let (pre_amount_modifier, after_modifier) = strip_measurement_modifier(&after_plus_prefix);
    let (amount, after_amount) = extract_amount(&after_modifier);
    let (pre_unit_modifier, after_pre_unit) = strip_measurement_modifier(&after_amount);
    let modifier = pre_unit_modifier.or(pre_amount_modifier).or(plus_prefix);
    let (unit, remaining) = extract_unit(&after_pre_unit);

    Some((amount?, unit?, remaining, modifier))
}

fn leading_measurement_consumes_all(s: &str) -> bool {
    let (_, after_pre_amount_modifier) = strip_measurement_modifier(s);
    let (amount, after_amount) = extract_amount(&after_pre_amount_modifier);
    let (_, after_pre_unit_modifier) = strip_measurement_modifier(&after_amount);
    let (unit, after_unit) = extract_unit(&after_pre_unit_modifier);

    amount.is_some() && unit.is_some() && after_unit.trim().is_empty()
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
        let Some(close_idx) = find_matching_closing_paren(&remaining, start).or_else(|| {
            remaining
                .get(start + 1..)
                .and_then(|tail| tail.find(')').map(|idx| start + 1 + idx))
        }) else {
            break;
        };
        let paren_content = match remaining.get(start + 1..close_idx) {
            Some(s) => s,
            None => break,
        };
        let paren_content = paren_content.trim_start_matches('(').trim();

        let raw_before_parenthetical = remaining.get(..start).unwrap_or("");
        let (follows_comma, follows_or) = parenthetical_branch_context(raw_before_parenthetical);
        let before_parenthetical = raw_before_parenthetical
            .trim_end()
            .trim_end_matches(',')
            .trim_end();
        let after_parenthetical = remaining
            .get(close_idx + 1..)
            .unwrap_or("")
            .trim_start()
            .trim_start_matches(')')
            .trim_start();

        if let Some((item_segment, note_segment)) = split_parenthetical_item_identity(paren_content)
        {
            let outside_text = join_segments(before_parenthetical, after_parenthetical);
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
                follows_or,
            );
            // Remove the parenthetical from remaining
            // Also strip trailing comma before the parenthetical (e.g., "onion, (diced)")
            remaining = join_segments(before_parenthetical, after_parenthetical);
            continue;
        }

        // Try to parse the parenthetical content as one or more measurements
        // Split by semicolons or commas to handle "8 ounces; 227 g each"
        let parsed_measurements = parse_parenthetical_measurement_details(paren_content);

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
        let before_plus = remaining.get(..plus_idx).unwrap_or("").trim();
        let after_plus = remaining.get(plus_idx + 6..).unwrap_or("").trim();
        if !after_plus.is_empty() {
            let (plus_prefix, _) = strip_plus_measurement_prefix(after_plus);
            let plus_measurement = parse_plus_continuation_measurement(after_plus);
            let plus_belongs_to_leading_measurement = leading_measurement_consumes_all(before_plus);

            // Only if we DON'T have amount+unit, treat as note (fallback)
            // Cases with valid measurement are handled later as compound amounts
            if plus_measurement.is_none()
                || (plus_prefix.is_some() && !plus_belongs_to_leading_measurement)
            {
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

    // Handle a "+" between the amount and the unit:
    // - "1 + 1/2 teaspoons" is a mixed number -> amount "1 1/2"
    // - "1/4 + 1/8 teaspoon" sums two fractions sharing a unit -> amount "1/4 plus 1/8"
    // - "3+ cups sugar" / "1/4 + teaspoon salt" has a stray "+" -> drop it
    if let Some(amount) = primary_amount.as_deref() {
        let remaining_trimmed = remaining.trim_start();
        if let Some(after_plus) = remaining_trimmed.strip_prefix('+') {
            let after_plus = after_plus.trim_start();
            let (next_amount, after_next_amount) = extract_amount(after_plus);
            match next_amount {
                Some(next) if next.contains('/') => {
                    primary_amount = if amount.chars().all(|c| c.is_ascii_digit()) {
                        Some(format!("{} {}", amount, next))
                    } else {
                        Some(format!("{} plus {}", amount, next))
                    };
                    remaining = after_next_amount;
                }
                None if extract_unit(after_plus).0.is_some() => {
                    remaining = after_plus.to_string();
                }
                _ => {}
            }
        }
    }

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

    // Step 4.4b: Handle "plus [amount] [unit]" compound quantities, including the
    // "+" spelling and chains like "3/4 cup plus 2 Tbsp. plus 1 tsp."
    // e.g., "1/2 cup plus 2 tablespoons flour" -> amount="1/2 cup plus 2 tablespoons", unit=null
    // This keeps the compound quantity together as a single amount rather than splitting into note
    loop {
        let remaining_trimmed = remaining.trim_start();
        let remaining_lower = remaining_trimmed.to_lowercase();
        let (after_plus, is_plus_sign) = if let Some(rest) = remaining_trimmed.strip_prefix('+') {
            (rest.trim_start(), true)
        } else if remaining_lower.starts_with("plus ") {
            (remaining_trimmed.get(5..).unwrap_or("").trim_start(), false)
        } else {
            break;
        };

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

    // Step 4.7b: Fold same-unit trailing "+ amount unit" continuations that
    // appear after the item text, e.g. "3/4 tsp salt + 1/8 tsp".
    if let (Some(amount), Some(unit)) = (primary_amount.as_ref(), primary_unit.as_ref()) {
        if let Some(plus_idx) = remaining.rfind(" + ") {
            let before_plus = remaining.get(..plus_idx).unwrap_or("").trim();
            let after_plus = remaining.get(plus_idx + 3..).unwrap_or("").trim();
            if !before_plus.is_empty() {
                if let Some((plus_amount, Some(plus_unit), after_plus_unit)) =
                    parse_range_continuation_measurement(after_plus)
                {
                    if after_plus_unit.trim().is_empty() && units_share_base(unit, &plus_unit) {
                        primary_amount = Some(format!("{} plus {}", amount, plus_amount));
                        remaining = before_plus.to_string();
                    }
                }
            }
        }
    }

    // Step 4.7c: Treat mid-item "+ more ..." guidance as a note, matching the
    // existing handling for ", plus more ...".
    if note.is_none() {
        if let Some(plus_idx) = remaining.to_lowercase().find(" + ") {
            let before_plus = remaining.get(..plus_idx).unwrap_or("").trim();
            let after_plus = remaining.get(plus_idx + 3..).unwrap_or("").trim();
            if !before_plus.is_empty() && !after_plus.is_empty() {
                let plus_note = format!("plus {}", after_plus);
                if is_trailing_guidance_note(&plus_note) {
                    note = Some(plus_note);
                    remaining = before_plus.to_string();
                }
            }
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
                if (is_trailing_prep_note(potential_note)
                    || is_trailing_guidance_note(potential_note))
                    && !is_only_prep_words(potential_item)
                {
                    let mut extracted_note = potential_note.to_string();
                    if is_trailing_guidance_note(potential_note) {
                        if let Some(branch_note) =
                            take_last_or_parenthetical_note(&mut deferred_parenthetical_notes)
                        {
                            extracted_note = format!("{} ({})", extracted_note, branch_note);
                        }
                    }
                    note = Some(extracted_note);
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
                let alternative_note =
                    take_last_or_parenthetical_note(&mut deferred_parenthetical_notes);
                note = Some(match alternative_note {
                    Some(alternative_note) => format!("or {} ({})", after_or, alternative_note),
                    None => format!("or {}", after_or),
                });
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

/// Parse multiple ingredient lines (separated by newlines).
/// Detects section headers (lines ending with colon, no measurements) and
/// applies the section name to subsequent ingredients.
/// Skips lines that should be ignored (scraper artifacts like "Gather Your Ingredients").
pub fn parse_ingredients(blob: &str) -> Vec<ParsedIngredient> {
    let mut current_section: Option<String> = None;
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
        assert_eq!(result[0].item, "envelopes unflavored powdered gelatin");
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
}
