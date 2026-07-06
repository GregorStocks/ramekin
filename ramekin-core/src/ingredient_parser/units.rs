//! Unit tables, unit extraction (compound/attached/multiplier), and normalization.

use super::*;

/// Common cooking units (lowercase for matching).
/// Sorted by length at runtime (longest first) to avoid partial matches
/// (e.g., "tablespoons" must match before "tb").
pub(super) static UNITS_SORTED: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut units = UNITS_RAW.to_vec();
    units.sort_by_key(|u| std::cmp::Reverse(u.len()));
    units
});

pub(super) const UNITS_RAW: &[&str] = &[
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
pub(super) static UNIT_CANONICAL_MAP: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
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
pub(super) const MEASUREMENT_MODIFIERS: &[&str] = &[
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

/// Strip measurement modifiers from the beginning of a string.
/// Returns (modifier if found, remaining_string).
pub(super) fn strip_measurement_modifier(s: &str) -> (Option<String>, String) {
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

pub(super) fn units_share_base(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
        || a.strip_suffix(&format!(" {}", b))
            .is_some_and(|prefix| !prefix.is_empty())
        || b.strip_suffix(&format!(" {}", a))
            .is_some_and(|prefix| !prefix.is_empty())
}

pub(super) fn extract_unit(s: &str) -> (Option<String>, String) {
    let s = s.trim();

    if let Some(remaining) = extract_single_letter_period_unit(s, "T.") {
        return (Some("tbsp".to_string()), remaining);
    }
    if let Some(remaining) = extract_single_letter_period_unit(s, "t.") {
        return (Some("tsp".to_string()), remaining);
    }

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

fn extract_single_letter_period_unit(s: &str, marker: &str) -> Option<String> {
    let after = s.strip_prefix(marker)?;
    if after.is_empty() || after.starts_with(|c: char| c.is_whitespace() || c == ',') {
        return Some(
            after
                .trim()
                .trim_start_matches("of ")
                .trim_start()
                .to_string(),
        );
    }
    None
}

/// Count/container nouns that pair with a hyphenated size descriptor.
/// Examples: "14-ounce package", "1/2-inch piece", "8-ounce block".
pub(super) const HYPHENATED_DESCRIPTOR_NOUNS: &[&str] = &[
    "package", "packages", "pkg", "pkgs", "can", "cans", "bag", "bags", "block", "blocks", "wheel",
    "wheels", "piece", "pieces", "knob", "knobs", "segment", "segments", "slice", "slices",
    "stick", "sticks", "loaf", "loaves", "hunk", "hunks",
];

/// Recover a measurement from a hyphenated tail left behind after the numeric
/// amount has already been extracted, e.g. "-ounce can" or "/2-inch piece".
pub(super) fn try_extract_hyphenated_unit_tail(
    amount: &str,
    s: &str,
) -> Option<((String, String), String)> {
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
pub(super) const CONTAINERS: &[&str] = &[
    "packages", "package", "bottles", "bottle", "boxes", "cans", "jars", "bags", "box", "can",
    "jar", "bag", "pkgs", "pkg",
];

/// Weight/volume units that can precede containers in compound units
pub(super) const WEIGHT_UNITS_FOR_COMPOUND: &[&str] = &[
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
pub(super) fn try_extract_compound_unit(s: &str) -> Option<(String, String)> {
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
pub(super) const ATTACHED_METRIC_UNITS: &[&str] = &["kg", "g", "mg", "ml", "l", "oz", "lb", "lbs"];

/// Try to extract a metric measurement attached to a number at the start of the string.
/// e.g., "65g granulated sugar" -> Some((Measurement{amount: "65", unit: "g"}, "granulated sugar"))
/// Also handles "120g/2.75 oz." format - extracts "120g" and leaves "/2.75 oz." for next iteration.
/// Returns None if no attached metric is found.
pub(super) fn try_extract_attached_metric(s: &str) -> Option<(Measurement, String)> {
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
pub(super) fn try_extract_multiplier_unit(s: &str) -> Option<(String, String)> {
    let s = s.trim_start();
    let after_marker = if let Some(rest) = s.strip_prefix('x') {
        rest
    } else {
        s.strip_prefix('×')?
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

/// Normalize a unit string to its canonical form.
///
/// Handles:
/// - Direct mappings: "cups" → "cup", "tablespoons" → "tbsp"
/// - Modifiers: "heaping cups" → "heaping cup"
/// - "each" suffix: "ounces each" → "oz each"
///
/// Returns the original unit if no normalization is needed.
pub(super) fn normalize_unit(unit: &str) -> String {
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
pub(super) fn normalize_unit_base(unit: &str) -> String {
    let unit_lower = unit.to_lowercase();
    if let Some(&canonical) = UNIT_CANONICAL_MAP.get(unit_lower.as_str()) {
        canonical.to_string()
    } else {
        unit.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_hyphenated_unit_tail_without_descriptor_noun() {
        let result = parse_ingredient("1-pound ground beef");
        assert_eq!(result.item, "ground beef");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("lb".to_string()));
    }
}
