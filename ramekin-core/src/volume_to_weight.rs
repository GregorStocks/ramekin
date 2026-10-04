//! Gram amounts computed for ingredient lines when a recipe is read.
//!
//! Converts oz/lb exactly, and volume measurements (cups, tbsp, tsp, etc.)
//! through the catalog's density for the food. The result is never stored, so
//! catalog improvements reach every recipe.

use crate::catalog::{is_volume_unit, line_grams_per_cup, volume_to_cups};
use crate::ingredient_parser::{normalize_fraction_to_decimal, Measurement};
use crate::metric_weights::{
    format_grams, has_metric_weight, metric_grams, parse_amount, MetricConversionStats,
};

/// Statistics about volume-to-weight conversion.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct VolumeConversionStats {
    pub converted: usize,
    pub skipped_no_volume: usize,
    pub skipped_unknown_ingredient: usize,
    pub skipped_already_has_weight: usize,
    pub skipped_unparseable: usize,
    /// Names of ingredients that had volume measurements but no density data.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unknown_ingredients: Vec<String>,
}

/// Grams for the first volume measurement, through the catalog's density for
/// the line's food, unless the line already has a weight.
fn volume_grams(
    item: &str,
    note: Option<&str>,
    measurements: &[Measurement],
    stats: &mut VolumeConversionStats,
) -> Option<String> {
    if has_weight_measurement(measurements) {
        stats.skipped_already_has_weight += 1;
        return None;
    }

    let Some((unit, amount)) = measurements.iter().find_map(|m| {
        let unit = m
            .unit
            .as_deref()
            .filter(|unit| is_volume_unit(Some(unit)))?;
        Some((unit, m.amount.as_deref()))
    }) else {
        stats.skipped_no_volume += 1;
        return None;
    };

    let Some(grams_per_cup) = line_grams_per_cup(item, note) else {
        stats.skipped_unknown_ingredient += 1;
        stats.unknown_ingredients.push(item.to_string());
        return None;
    };

    let Some(grams) =
        amount.and_then(|amount| convert_volume_to_grams(amount, unit, grams_per_cup))
    else {
        stats.skipped_unparseable += 1;
        return None;
    };

    stats.converted += 1;
    Some(grams)
}

/// Whether any measurement is already a weight, metric or oz/lb.
fn has_weight_measurement(measurements: &[Measurement]) -> bool {
    has_metric_weight(measurements)
        || measurements
            .iter()
            .any(|m| matches!(m.unit.as_deref(), Some("oz") | Some("lb")))
}

/// Convert a volume amount to grams.
///
/// Handles simple amounts (no ranges for now - volume measurements rarely have ranges).
fn convert_volume_to_grams(amount: &str, unit: &str, grams_per_cup: f64) -> Option<String> {
    let value = parse_amount(amount)?;
    let cups = volume_to_cups(value, unit)?;
    let grams = cups * grams_per_cup;
    Some(format_grams(grams))
}

/// The gram amount computed for an ingredient line, shown beside what the
/// source gave and never stored. It converts oz/lb exactly, or a volume
/// through the catalog's density for the food. None when a weight is already
/// metric, or when nothing converts.
pub fn derived_grams(
    item: &str,
    note: Option<&str>,
    measurements: &[Measurement],
) -> Option<Measurement> {
    derived_grams_with_stats(
        item,
        note,
        measurements,
        &mut MetricConversionStats::default(),
        &mut VolumeConversionStats::default(),
    )
}

/// `derived_grams`, counting why each line did or didn't convert.
pub fn derived_grams_with_stats(
    item: &str,
    note: Option<&str>,
    measurements: &[Measurement],
    metric_stats: &mut MetricConversionStats,
    volume_stats: &mut VolumeConversionStats,
) -> Option<Measurement> {
    // Both always run so each counts every line. They never both convert: a
    // line with oz/lb already has a weight for the volume conversion.
    let metric = metric_grams(measurements, metric_stats);
    let volume = volume_grams(item, note, measurements, volume_stats);
    let grams = metric.or(volume)?;
    Some(Measurement {
        amount: Some(normalize_fraction_to_decimal(&grams)),
        unit: Some("g".to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measurements(pairs: &[(&str, &str)]) -> Vec<Measurement> {
        pairs
            .iter()
            .map(|(amount, unit)| Measurement {
                amount: Some(amount.to_string()),
                unit: Some(unit.to_string()),
            })
            .collect()
    }

    fn derived(item: &str, pairs: &[(&str, &str)]) -> Option<Measurement> {
        derived_grams(item, None, &measurements(pairs))
    }

    fn grams(amount: &str) -> Option<Measurement> {
        Some(Measurement {
            amount: Some(amount.to_string()),
            unit: Some("g".to_string()),
        })
    }

    #[test]
    fn derived_grams_converts_volume_through_density() {
        // 2 * 125 g
        assert_eq!(derived("all-purpose flour", &[("2", "cup")]), grams("250"));
        // 2 tbsp = 0.125 cup; 0.125 * 200 g
        assert_eq!(derived("sugar", &[("2", "tbsp")]), grams("25"));
        // 0.5 cup * 227 g = 113.5 g
        assert_eq!(derived("unsalted butter", &[("1/2", "cup")]), grams("114"));
        assert_eq!(derived("softened butter", &[("1", "cup")]), grams("227"));
    }

    #[test]
    fn derived_grams_converts_imperial_weight() {
        assert_eq!(derived("butter", &[("8", "oz")]), grams("227"));
    }

    #[test]
    fn derived_grams_prefers_a_weight_over_a_volume() {
        // 4 oz, not the density of 1/2 cup of butter (114 g).
        assert_eq!(
            derived("butter", &[("0.5", "cup"), ("4", "oz")]),
            grams("113")
        );
    }

    #[test]
    fn derived_grams_normalizes_like_stored_amounts() {
        assert_eq!(derived("butter", &[("1/4", "oz")]), grams("7.1"));
        // The gram formatter's "5.0", normalized like a parsed amount.
        assert_eq!(derived("butter", &[("0.17637", "oz")]), grams("5"));
    }

    #[test]
    fn derived_grams_skips_lines_with_metric_weight() {
        assert_eq!(derived("flour", &[("1", "cup"), ("120", "g")]), None);
        assert_eq!(derived("flour", &[("1", "cup"), ("120", "grams")]), None);
    }

    #[test]
    fn derived_grams_skips_unknown_foods_and_counts() {
        assert_eq!(derived("unicorn tears", &[("1", "cup")]), None);
        assert_eq!(derived("egg", &[("2", "")]), None);
    }

    #[test]
    fn derived_grams_counts_stats() {
        let mut metric = MetricConversionStats::default();
        let mut volume = VolumeConversionStats::default();
        let mut count = |item: &str, pairs: &[(&str, &str)]| {
            derived_grams_with_stats(item, None, &measurements(pairs), &mut metric, &mut volume)
        };
        count("sugar", &[("2", "tbsp")]);
        count("butter", &[("8", "oz")]);
        count("unicorn tears", &[("1", "cup")]);
        count("flour", &[("1", "cup"), ("125", "g")]);
        count("egg", &[("2", "")]);
        assert_eq!(volume.converted, 1);
        assert_eq!(volume.skipped_already_has_weight, 2);
        assert_eq!(volume.skipped_unknown_ingredient, 1);
        assert_eq!(volume.unknown_ingredients, vec!["unicorn tears"]);
        assert_eq!(volume.skipped_no_volume, 1);
        assert_eq!(metric.converted_oz, 1);
        assert_eq!(metric.skipped_already_metric, 1);
    }
}
