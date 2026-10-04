//! Gram amounts computed for ingredient lines when a recipe is read.
//!
//! Converts oz/lb exactly, and volume measurements (cups, tbsp, tsp, etc.)
//! through the catalog's density for the food. The result is never stored, so
//! catalog improvements reach every recipe.

use crate::catalog::{is_volume_unit, line_grams_per_cup, volume_to_cups};
use crate::ingredient_parser::{Measurement, ParsedIngredient};
use crate::metric_weights::{
    add_metric_weight_alternative, format_grams, parse_amount, MetricConversionStats,
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

/// Add a weight measurement alternative for volume-measured ingredients.
///
/// Only converts when:
/// 1. The ingredient has a volume measurement (cup, tbsp, tsp, etc.)
/// 2. No weight measurement already exists
/// 3. The ingredient name matches a known density entry
fn add_volume_to_weight_alternative(
    mut ingredient: ParsedIngredient,
    stats: &mut VolumeConversionStats,
) -> ParsedIngredient {
    // Check if already has weight measurement
    if has_weight_measurement(&ingredient.measurements) {
        stats.skipped_already_has_weight += 1;
        return ingredient;
    }

    // Find first volume measurement and extract info
    let volume_info = ingredient.measurements.iter().find_map(|m| {
        if is_volume_unit(m.unit.as_deref()) {
            Some((m.unit.clone(), m.amount.clone()))
        } else {
            None
        }
    });

    let Some((unit_opt, amount_opt)) = volume_info else {
        stats.skipped_no_volume += 1;
        return ingredient;
    };

    let Some(unit) = unit_opt else {
        stats.skipped_no_volume += 1;
        return ingredient;
    };

    // Look up density for this ingredient
    let Some(grams_per_cup) = line_grams_per_cup(&ingredient.item, ingredient.note.as_deref())
    else {
        stats.skipped_unknown_ingredient += 1;
        stats.unknown_ingredients.push(ingredient.item.clone());
        return ingredient;
    };

    let Some(amount_str) = amount_opt else {
        stats.skipped_unparseable += 1;
        return ingredient;
    };

    // Convert the amount to grams
    let gram_amount = match convert_volume_to_grams(&amount_str, &unit, grams_per_cup) {
        Some(g) => g,
        None => {
            stats.skipped_unparseable += 1;
            return ingredient;
        }
    };

    // Add the weight alternative
    ingredient.measurements.push(Measurement {
        amount: Some(gram_amount),
        unit: Some("g".to_string()),
    });

    stats.converted += 1;
    ingredient
}

/// Check if any measurement already has a weight unit.
fn has_weight_measurement(measurements: &[Measurement]) -> bool {
    measurements.iter().any(|m| {
        matches!(
            m.unit.as_deref(),
            Some("g") | Some("kg") | Some("mg") | Some("oz") | Some("lb")
        )
    })
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
pub fn derived_grams(ingredient: &ParsedIngredient) -> Option<Measurement> {
    derived_grams_with_stats(
        ingredient,
        &mut MetricConversionStats::default(),
        &mut VolumeConversionStats::default(),
    )
}

/// `derived_grams`, counting why each line did or didn't convert.
pub fn derived_grams_with_stats(
    ingredient: &ParsedIngredient,
    metric_stats: &mut MetricConversionStats,
    volume_stats: &mut VolumeConversionStats,
) -> Option<Measurement> {
    let probe = ParsedIngredient {
        item: ingredient.item.clone(),
        measurements: ingredient.measurements.clone(),
        note: ingredient.note.clone(),
        raw: None,
        section: None,
    };
    let probe = add_metric_weight_alternative(probe, metric_stats);
    let mut probe = add_volume_to_weight_alternative(probe, volume_stats).normalize_amounts();
    if probe.measurements.len() == ingredient.measurements.len() {
        return None;
    }
    probe.measurements.pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_cup_flour() {
        let ingredient = ParsedIngredient {
            item: "all-purpose flour".to_string(),
            measurements: vec![Measurement {
                amount: Some("2".to_string()),
                unit: Some("cup".to_string()),
            }],
            note: None,
            raw: Some("2 cups all-purpose flour".to_string()),
            section: None,
        };

        let mut stats = VolumeConversionStats::default();
        let result = add_volume_to_weight_alternative(ingredient, &mut stats);

        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[1].amount, Some("250".to_string())); // 2 * 125g
        assert_eq!(result.measurements[1].unit, Some("g".to_string()));
        assert_eq!(stats.converted, 1);
    }

    #[test]
    fn test_convert_tbsp_sugar() {
        let ingredient = ParsedIngredient {
            item: "sugar".to_string(),
            measurements: vec![Measurement {
                amount: Some("2".to_string()),
                unit: Some("tbsp".to_string()),
            }],
            note: None,
            raw: Some("2 tbsp sugar".to_string()),
            section: None,
        };

        let mut stats = VolumeConversionStats::default();
        let result = add_volume_to_weight_alternative(ingredient, &mut stats);

        assert_eq!(result.measurements.len(), 2);
        // 2 tbsp = 2/16 cup = 0.125 cup; 0.125 * 200g = 25g
        assert_eq!(result.measurements[1].amount, Some("25".to_string()));
        assert_eq!(stats.converted, 1);
    }

    #[test]
    fn test_convert_with_alias() {
        let ingredient = ParsedIngredient {
            item: "unsalted butter".to_string(),
            measurements: vec![Measurement {
                amount: Some("1/2".to_string()),
                unit: Some("cup".to_string()),
            }],
            note: None,
            raw: Some("1/2 cup unsalted butter".to_string()),
            section: None,
        };

        let mut stats = VolumeConversionStats::default();
        let result = add_volume_to_weight_alternative(ingredient, &mut stats);

        assert_eq!(result.measurements.len(), 2);
        // 0.5 cup * 227g = 113.5g -> 114g
        assert_eq!(result.measurements[1].amount, Some("114".to_string()));
        assert_eq!(stats.converted, 1);
    }

    #[test]
    fn test_skip_unknown_ingredient() {
        let ingredient = ParsedIngredient {
            item: "unicorn tears".to_string(),
            measurements: vec![Measurement {
                amount: Some("1".to_string()),
                unit: Some("cup".to_string()),
            }],
            note: None,
            raw: Some("1 cup unicorn tears".to_string()),
            section: None,
        };

        let mut stats = VolumeConversionStats::default();
        let result = add_volume_to_weight_alternative(ingredient, &mut stats);

        assert_eq!(result.measurements.len(), 1);
        assert_eq!(stats.skipped_unknown_ingredient, 1);
    }

    #[test]
    fn test_skip_already_has_weight() {
        let ingredient = ParsedIngredient {
            item: "flour".to_string(),
            measurements: vec![
                Measurement {
                    amount: Some("1".to_string()),
                    unit: Some("cup".to_string()),
                },
                Measurement {
                    amount: Some("125".to_string()),
                    unit: Some("g".to_string()),
                },
            ],
            note: None,
            raw: Some("1 cup (125g) flour".to_string()),
            section: None,
        };

        let mut stats = VolumeConversionStats::default();
        let result = add_volume_to_weight_alternative(ingredient, &mut stats);

        assert_eq!(result.measurements.len(), 2); // unchanged
        assert_eq!(stats.skipped_already_has_weight, 1);
    }

    #[test]
    fn test_skip_no_volume() {
        // Test with a count-based measurement (no volume, no weight)
        let ingredient = ParsedIngredient {
            item: "egg".to_string(),
            measurements: vec![Measurement {
                amount: Some("2".to_string()),
                unit: None,
            }],
            note: None,
            raw: Some("2 eggs".to_string()),
            section: None,
        };

        let mut stats = VolumeConversionStats::default();
        let result = add_volume_to_weight_alternative(ingredient, &mut stats);

        assert_eq!(result.measurements.len(), 1);
        assert_eq!(stats.skipped_no_volume, 1);
    }

    #[test]
    fn test_convert_with_modifier_in_name() {
        let ingredient = ParsedIngredient {
            item: "softened butter".to_string(),
            measurements: vec![Measurement {
                amount: Some("1".to_string()),
                unit: Some("cup".to_string()),
            }],
            note: None,
            raw: Some("1 cup softened butter".to_string()),
            section: None,
        };

        let mut stats = VolumeConversionStats::default();
        let result = add_volume_to_weight_alternative(ingredient, &mut stats);

        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[1].amount, Some("227".to_string()));
        assert_eq!(stats.converted, 1);
    }

    fn line(item: &str, measurements: &[(&str, &str)]) -> ParsedIngredient {
        ParsedIngredient {
            item: item.to_string(),
            measurements: measurements
                .iter()
                .map(|(amount, unit)| Measurement {
                    amount: Some(amount.to_string()),
                    unit: Some(unit.to_string()),
                })
                .collect(),
            note: None,
            raw: None,
            section: None,
        }
    }

    fn grams(amount: &str) -> Option<Measurement> {
        Some(Measurement {
            amount: Some(amount.to_string()),
            unit: Some("g".to_string()),
        })
    }

    #[test]
    fn derived_grams_converts_volume_through_density() {
        assert_eq!(
            derived_grams(&line("all-purpose flour", &[("2", "cup")])),
            grams("250")
        );
    }

    #[test]
    fn derived_grams_converts_imperial_weight() {
        assert_eq!(derived_grams(&line("butter", &[("8", "oz")])), grams("227"));
    }

    #[test]
    fn derived_grams_prefers_a_weight_over_a_volume() {
        // 4 oz, not the density of 1/2 cup of butter (114 g).
        assert_eq!(
            derived_grams(&line("butter", &[("0.5", "cup"), ("4", "oz")])),
            grams("113")
        );
    }

    #[test]
    fn derived_grams_normalizes_like_stored_amounts() {
        assert_eq!(
            derived_grams(&line("butter", &[("1/4", "oz")])),
            grams("7.1")
        );
    }

    #[test]
    fn derived_grams_skips_lines_with_metric_weight() {
        assert_eq!(
            derived_grams(&line("flour", &[("1", "cup"), ("120", "g")])),
            None
        );
    }

    #[test]
    fn derived_grams_skips_unknown_foods_and_counts() {
        assert_eq!(derived_grams(&line("unicorn tears", &[("1", "cup")])), None);
        assert_eq!(derived_grams(&line("egg", &[("2", "")])), None);
    }

    #[test]
    fn derived_grams_counts_stats() {
        let mut metric = MetricConversionStats::default();
        let mut volume = VolumeConversionStats::default();
        derived_grams_with_stats(&line("sugar", &[("2", "tbsp")]), &mut metric, &mut volume);
        derived_grams_with_stats(&line("butter", &[("8", "oz")]), &mut metric, &mut volume);
        assert_eq!(volume.converted, 1);
        assert_eq!(metric.converted_oz, 1);
    }
}
