//! Metric weight conversion module.
//!
//! Provides deterministic conversion of imperial weight measurements to metric.
//! Handles oz → grams and lb → grams conversions.

use crate::ingredient_parser::Measurement;

const GRAMS_PER_OZ: f64 = 28.3495;
const GRAMS_PER_LB: f64 = 453.592;

/// Statistics about metric weight conversion.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricConversionStats {
    pub converted_oz: usize,
    pub converted_lb: usize,
    pub skipped_no_us_weight: usize,
    pub skipped_already_metric: usize,
    pub skipped_unparseable: usize,
}

/// Grams for the first oz or lb measurement, unless one is already metric.
pub(crate) fn metric_grams(
    measurements: &[Measurement],
    stats: &mut MetricConversionStats,
) -> Option<String> {
    if has_metric_weight(measurements) {
        stats.skipped_already_metric += 1;
        return None;
    }

    let Some((unit, grams_per_unit, amount)) =
        measurements.iter().find_map(|m| match m.unit.as_deref() {
            Some("oz") => Some(("oz", GRAMS_PER_OZ, m.amount.as_deref())),
            Some("lb") => Some(("lb", GRAMS_PER_LB, m.amount.as_deref())),
            _ => None,
        })
    else {
        stats.skipped_no_us_weight += 1;
        return None;
    };

    let Some(grams) = amount.and_then(|amount| convert_amount_to_grams(amount, grams_per_unit))
    else {
        stats.skipped_unparseable += 1;
        return None;
    };

    if unit == "lb" {
        stats.converted_lb += 1;
    } else {
        stats.converted_oz += 1;
    }
    Some(grams)
}

/// Whether any measurement already has a metric weight unit.
pub(crate) fn has_metric_weight(measurements: &[Measurement]) -> bool {
    measurements
        .iter()
        .any(|m| is_metric_weight_unit(m.unit.as_deref()))
}

/// Whether a unit is a metric weight ("g", "kilograms", ...).
pub fn is_metric_weight_unit(unit: Option<&str>) -> bool {
    matches!(
        unit,
        Some("g")
            | Some("kg")
            | Some("mg")
            | Some("gram")
            | Some("grams")
            | Some("kilogram")
            | Some("kilograms")
    )
}

/// Convert an amount string to grams, returning formatted string.
///
/// Handles:
/// - Simple numbers: "8" → "227"
/// - Decimals: "2.5" → "71"
/// - Fractions: "1/2" → "14"
/// - Mixed numbers: "1 1/2" → "43"
/// - Ranges with hyphen: "6-8" → "170-227"
/// - Ranges with "to": "6 to 8" → "170 to 227"
fn convert_amount_to_grams(amount: &str, grams_per_unit: f64) -> Option<String> {
    let amount = amount.trim();

    // Check for range with " to "
    if let Some((low, high)) = amount.split_once(" to ") {
        let low_g = parse_and_convert(low.trim(), grams_per_unit)?;
        let high_g = parse_and_convert(high.trim(), grams_per_unit)?;
        return Some(format!(
            "{} to {}",
            format_grams(low_g),
            format_grams(high_g)
        ));
    }

    // Check for range with " or "
    if let Some((low, high)) = amount.split_once(" or ") {
        let low_g = parse_and_convert(low.trim(), grams_per_unit)?;
        let high_g = parse_and_convert(high.trim(), grams_per_unit)?;
        return Some(format!(
            "{} or {}",
            format_grams(low_g),
            format_grams(high_g)
        ));
    }

    // Check for range with hyphen (but not negative numbers or fractions like 1-1/2)
    // A range hyphen should have digits on both sides: "6-8"
    if let Some(hyphen_idx) = find_range_hyphen(amount) {
        let low = amount.get(..hyphen_idx)?;
        let high = amount.get(hyphen_idx + 1..)?;
        let low_g = parse_and_convert(low.trim(), grams_per_unit)?;
        let high_g = parse_and_convert(high.trim(), grams_per_unit)?;
        return Some(format!("{}-{}", format_grams(low_g), format_grams(high_g)));
    }

    // Single amount
    let grams = parse_and_convert(amount, grams_per_unit)?;
    Some(format_grams(grams))
}

/// Find the index of a range hyphen (not a fraction hyphen like in "1-1/2").
fn find_range_hyphen(s: &str) -> Option<usize> {
    // Look for pattern: digit-digit where the part after hyphen doesn't contain /
    for (i, c) in s.char_indices() {
        if c == '-' && i > 0 {
            let before = s.get(..i)?.chars().last()?;
            let after_part = s.get(i + 1..)?;
            let after = after_part.chars().next()?;

            // Must have digit before and digit after
            if before.is_ascii_digit() && after.is_ascii_digit() {
                // The part after must not contain a slash (would be mixed number like 1-1/2)
                if !after_part.contains('/') {
                    return Some(i);
                }
            }
        }
    }
    None
}

/// Parse an amount string and convert to grams.
fn parse_and_convert(amount: &str, grams_per_unit: f64) -> Option<f64> {
    let value = parse_amount(amount)?;
    Some(value * grams_per_unit)
}

/// Parse an amount string into a decimal value.
///
/// Handles:
/// - Integers: "8" → 8.0
/// - Decimals: "2.5" → 2.5
/// - Fractions: "1/2" → 0.5
/// - Mixed numbers: "1 1/2" → 1.5
pub fn parse_amount(amount: &str) -> Option<f64> {
    let amount = amount.trim();

    if amount.is_empty() {
        return None;
    }

    // Try mixed number: "1 1/2" or "2 3/4"
    let parts: Vec<&str> = amount.split_whitespace().collect();
    if parts.len() == 2 {
        let whole: f64 = parts[0].parse().ok()?;
        let frac = parse_fraction(parts[1])?;
        return Some(whole + frac);
    }

    // Try fraction: "1/2"
    if amount.contains('/') {
        return parse_fraction(amount);
    }

    // Try decimal or integer
    amount.parse().ok()
}

/// Parse a fraction string like "1/2" or "3/4".
fn parse_fraction(s: &str) -> Option<f64> {
    let (num, denom) = s.split_once('/')?;
    let num: f64 = num.trim().parse().ok()?;
    let denom: f64 = denom.trim().parse().ok()?;
    if denom == 0.0 {
        return None;
    }
    Some(num / denom)
}

/// Format grams as a string.
/// For amounts under 10g, shows one decimal place for precision.
/// For larger amounts, rounds to nearest whole number.
pub fn format_grams(grams: f64) -> String {
    if grams < 10.0 {
        format!("{:.1}", grams)
    } else {
        format!("{:.0}", grams)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_amount_integer() {
        assert_eq!(parse_amount("8"), Some(8.0));
        assert_eq!(parse_amount("12"), Some(12.0));
    }

    #[test]
    fn test_parse_amount_decimal() {
        assert_eq!(parse_amount("2.5"), Some(2.5));
        assert_eq!(parse_amount("0.25"), Some(0.25));
    }

    #[test]
    fn test_parse_amount_fraction() {
        assert_eq!(parse_amount("1/2"), Some(0.5));
        assert_eq!(parse_amount("3/4"), Some(0.75));
        assert_eq!(parse_amount("1/4"), Some(0.25));
    }

    #[test]
    fn test_parse_amount_mixed_number() {
        assert_eq!(parse_amount("1 1/2"), Some(1.5));
        assert_eq!(parse_amount("2 3/4"), Some(2.75));
    }

    #[test]
    fn test_convert_simple_oz() {
        // 8 oz = 227g (rounded)
        assert_eq!(
            convert_amount_to_grams("8", GRAMS_PER_OZ),
            Some("227".to_string())
        );
    }

    #[test]
    fn test_convert_simple_lb() {
        // 2 lb = 907g (rounded)
        assert_eq!(
            convert_amount_to_grams("2", GRAMS_PER_LB),
            Some("907".to_string())
        );
    }

    #[test]
    fn test_convert_fraction() {
        // 1/2 oz = 14g (rounded)
        assert_eq!(
            convert_amount_to_grams("1/2", GRAMS_PER_OZ),
            Some("14".to_string())
        );
    }

    #[test]
    fn test_convert_range_hyphen() {
        // 6-8 oz = 170-227g
        assert_eq!(
            convert_amount_to_grams("6-8", GRAMS_PER_OZ),
            Some("170-227".to_string())
        );
    }

    #[test]
    fn test_convert_range_to() {
        // 6 to 8 oz = 170 to 227g
        assert_eq!(
            convert_amount_to_grams("6 to 8", GRAMS_PER_OZ),
            Some("170 to 227".to_string())
        );
    }

    #[test]
    fn test_convert_range_or() {
        assert_eq!(
            convert_amount_to_grams("6 or 8", GRAMS_PER_OZ),
            Some("170 or 227".to_string())
        );
    }

    fn measurements(pairs: &[(&str, &str)]) -> Vec<Measurement> {
        pairs
            .iter()
            .map(|(amount, unit)| Measurement {
                amount: Some(amount.to_string()),
                unit: Some(unit.to_string()),
            })
            .collect()
    }

    #[test]
    fn test_metric_grams_oz() {
        let mut stats = MetricConversionStats::default();
        let grams = metric_grams(&measurements(&[("8", "oz")]), &mut stats);
        assert_eq!(grams, Some("227".to_string()));
        assert_eq!(stats.converted_oz, 1);
    }

    #[test]
    fn test_metric_grams_lb() {
        let mut stats = MetricConversionStats::default();
        let grams = metric_grams(&measurements(&[("2", "lb")]), &mut stats);
        assert_eq!(grams, Some("907".to_string()));
        assert_eq!(stats.converted_lb, 1);
    }

    #[test]
    fn test_skip_non_us_weight() {
        let mut stats = MetricConversionStats::default();
        assert_eq!(
            metric_grams(&measurements(&[("2", "cups")]), &mut stats),
            None
        );
        assert_eq!(stats.skipped_no_us_weight, 1);
    }

    #[test]
    fn test_skip_already_metric() {
        let mut stats = MetricConversionStats::default();
        let grams = metric_grams(&measurements(&[("8", "oz"), ("227", "grams")]), &mut stats);
        assert_eq!(grams, None);
        assert_eq!(stats.skipped_already_metric, 1);
    }

    #[test]
    fn test_range_preserved() {
        let mut stats = MetricConversionStats::default();
        let grams = metric_grams(&measurements(&[("6-8", "oz")]), &mut stats);
        assert_eq!(grams, Some("170-227".to_string()));
    }
}
