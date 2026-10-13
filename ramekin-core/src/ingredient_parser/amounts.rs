//! Amount extraction and normalization: numbers, fractions, words, ranges.

use super::*;

/// Format a decimal amount, stripping trailing zeros.
/// "0.50" -> "0.5", "1.00" -> "1", "2.50" -> "2.5"
pub(super) fn format_decimal_amount(value: f64) -> String {
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
pub(super) fn normalize_single_amount(s: &str) -> String {
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
pub(super) fn find_range_hyphen_in_amount(s: &str) -> Option<usize> {
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
pub(crate) fn normalize_fraction_to_decimal(amount: &str) -> String {
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
pub(super) fn normalize_word_numbers(s: &str) -> String {
    // Handle fractional words first (before whole numbers)
    let fraction_to_digit = [("half", "1/2"), ("quarter", "1/4")];

    let s_lower = s.to_lowercase();
    // Names that start with a number word: "five spice powder", "half and half".
    const NUMBER_WORD_NAMES: &[&str] = &["five spice", "half and half", "half & half"];
    if NUMBER_WORD_NAMES.iter().any(|name| {
        s_lower
            .strip_prefix(name)
            .is_some_and(|after| !after.starts_with(char::is_alphanumeric))
    }) {
        return s.to_string();
    }
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

/// Extract an amount from the beginning of a string.
/// Returns (amount, remaining_string).
/// Handles ranges like "1 to 4" or "6 to 8" as well as simple amounts.
pub(super) fn extract_amount(s: &str) -> (Option<String>, String) {
    measure::read_amount(s)
}

pub(super) fn split_hyphenated_mixed_number(s: &str) -> Option<(&str, &str)> {
    let (whole, fraction) = s.split_once('-')?;
    if whole.chars().all(|c| c.is_ascii_digit()) && is_fraction(fraction) {
        Some((whole, fraction))
    } else {
        None
    }
}

/// Check if a string looks like an amount (number, fraction, decimal)
pub(super) fn is_amount_like(s: &str) -> bool {
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
pub(super) fn is_fraction(s: &str) -> bool {
    if let Some((before, after)) = s.split_once('/') {
        !before.is_empty()
            && !after.is_empty()
            && before.chars().all(|c| c.is_ascii_digit())
            && after.chars().all(|c| c.is_ascii_digit())
    } else {
        false
    }
}

pub(super) fn parse_range_continuation_measurement(
    s: &str,
) -> Option<(String, Option<String>, String)> {
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

pub(super) fn try_extract_hyphenated_descriptor_range(s: &str) -> Option<(String, String, String)> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unicode_fraction_slash_mixed_number() {
        let result = parse_ingredient("1 1⁄2 cups crushed kettle-style potato chips");
        assert_eq!(result.item, "crushed kettle-style potato chips");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1 1/2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
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
        // "1-1/2" (no space, fraction after hyphen) is the mixed number
        // 1 1/2, not a range.
        let result = parse_ingredient("1-1/2 cups sugar");
        assert_eq!(result.item, "sugar");
        assert_eq!(result.measurements[0].amount, Some("1 1/2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }
}
