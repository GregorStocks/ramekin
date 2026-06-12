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
pub(super) fn normalize_fraction_to_decimal(amount: &str) -> String {
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
pub(super) fn parse_leading_amount_words(words: &[&str]) -> Option<(String, usize)> {
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

pub(super) fn split_hyphenated_mixed_number(s: &str) -> Option<(&str, &str)> {
    let (whole, fraction) = s.split_once('-')?;
    if whole.chars().all(|c| c.is_ascii_digit()) && is_fraction(fraction) {
        Some((whole, fraction))
    } else {
        None
    }
}

pub(super) fn split_leading_attached_unit_token(s: &str) -> Option<(&str, &str)> {
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

/// Extract a unit from the beginning of a string.
/// Returns (unit, remaining_string).
pub(super) fn slash_starts_measurement(s: &str) -> bool {
    let Some(after_slash) = s.strip_prefix('/') else {
        return false;
    };
    let after_slash = after_slash.trim_start();
    let (amount, _) = extract_amount(after_slash);
    amount.is_some()
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
