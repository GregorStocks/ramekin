//! Ingredient density data for volume-to-weight conversion.
//!
//! Densities are stored as grams per US cup (236.588 ml).
//! Data sourced from USDA FoodData Central (public domain, CC0).

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Volume conversion factors to cups (the reference unit for density data).
pub const CUPS_PER_TBSP: f64 = 1.0 / 16.0;
pub const CUPS_PER_TSP: f64 = 1.0 / 48.0;
pub const CUPS_PER_FL_OZ: f64 = 1.0 / 8.0;
pub const CUPS_PER_PINT: f64 = 2.0;
pub const CUPS_PER_QUART: f64 = 4.0;
pub const CUPS_PER_GALLON: f64 = 16.0;
pub const CUPS_PER_ML: f64 = 1.0 / 236.588;
pub const CUPS_PER_L: f64 = 1000.0 / 236.588;

/// Raw density data loaded from JSON.
#[derive(Deserialize)]
struct DensityDataFile {
    ingredients: HashMap<String, f64>,
    aliases: HashMap<String, String>,
}

/// The embedded JSON data file.
static DATA_JSON: &str = include_str!("density_data.json");

/// Parsed density data.
static DATA: LazyLock<DensityDataFile> = LazyLock::new(|| {
    serde_json::from_str(DATA_JSON).expect("density_data.json should be valid JSON")
});

/// Common modifiers to strip from ingredient names before matching.
const MODIFIERS_TO_STRIP: &[&str] = &[
    // Temperature/state modifiers (prefix)
    "room temperature ",
    "cold ",
    "warm ",
    "melted ",
    "softened ",
    // Preparation modifiers (suffix)
    ", softened",
    ", melted",
    ", cold",
    ", at room temperature",
    ", room temperature",
    ", chilled",
    ", sifted",
];

/// Convert a volume unit to its equivalent in cups.
pub fn volume_to_cups(amount: f64, unit: &str) -> Option<f64> {
    match unit {
        "cup" => Some(amount),
        "tbsp" => Some(amount * CUPS_PER_TBSP),
        "tsp" => Some(amount * CUPS_PER_TSP),
        "fl oz" => Some(amount * CUPS_PER_FL_OZ),
        "pint" => Some(amount * CUPS_PER_PINT),
        "quart" => Some(amount * CUPS_PER_QUART),
        "gallon" => Some(amount * CUPS_PER_GALLON),
        "ml" => Some(amount * CUPS_PER_ML),
        "l" => Some(amount * CUPS_PER_L),
        _ => None,
    }
}

/// Check if a unit is a volume unit we can convert.
pub fn is_volume_unit(unit: Option<&str>) -> bool {
    matches!(
        unit,
        Some("cup")
            | Some("tbsp")
            | Some("tsp")
            | Some("fl oz")
            | Some("pint")
            | Some("quart")
            | Some("gallon")
            | Some("ml")
            | Some("l")
    )
}

/// Find the density (grams per cup) for an ingredient name.
///
/// Tries:
/// 1. Direct lookup in ingredients
/// 2. Lookup via aliases
/// 3. After stripping common modifiers, retry both lookups
pub fn find_density(ingredient_item: &str) -> Option<f64> {
    let normalized = normalize_ingredient_name(ingredient_item);

    // Direct lookup
    if let Some(&density) = DATA.ingredients.get(&normalized) {
        return Some(density);
    }

    // Alias lookup
    if let Some(canonical) = DATA.aliases.get(&normalized) {
        if let Some(&density) = DATA.ingredients.get(canonical) {
            return Some(density);
        }
    }

    // Try with modifiers stripped
    let stripped = strip_modifiers(&normalized);
    if stripped != normalized {
        if let Some(&density) = DATA.ingredients.get(&stripped) {
            return Some(density);
        }
        if let Some(canonical) = DATA.aliases.get(&stripped) {
            if let Some(&density) = DATA.ingredients.get(canonical) {
                return Some(density);
            }
        }
    }

    None
}

/// Normalize ingredient name for matching.
fn normalize_ingredient_name(s: &str) -> String {
    s.to_lowercase().trim().to_string()
}

/// Strip common modifiers from ingredient name.
fn strip_modifiers(s: &str) -> String {
    let mut result = s.to_string();
    for modifier in MODIFIERS_TO_STRIP {
        if let Some(stripped) = result.strip_prefix(modifier) {
            result = stripped.to_string();
        }
        if let Some(stripped) = result.strip_suffix(modifier) {
            result = stripped.to_string();
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_density_direct() {
        // These should find densities (exact names from USDA)
        assert!(find_density("cheese, cheddar").is_some());
    }

    #[test]
    fn test_find_density_alias() {
        // Common aliases should work
        assert!(find_density("flour").is_some());
        assert!(find_density("sugar").is_some());
        assert!(find_density("butter").is_some());
    }

    #[test]
    fn test_find_density_with_modifiers() {
        assert!(find_density("softened butter").is_some());
        assert!(find_density("melted butter").is_some());
    }

    #[test]
    fn test_find_density_case_insensitive() {
        assert!(find_density("FLOUR").is_some());
        assert!(find_density("Butter").is_some());
    }

    #[test]
    fn test_find_density_unknown() {
        assert_eq!(find_density("unicorn tears"), None);
        assert_eq!(find_density("mystery powder"), None);
    }

    #[test]
    fn test_volume_to_cups() {
        assert_eq!(volume_to_cups(1.0, "cup"), Some(1.0));
        assert_eq!(volume_to_cups(16.0, "tbsp"), Some(1.0));
        assert_eq!(volume_to_cups(48.0, "tsp"), Some(1.0));
        assert_eq!(volume_to_cups(8.0, "fl oz"), Some(1.0));
        assert_eq!(volume_to_cups(0.5, "pint"), Some(1.0));
    }

    #[test]
    fn test_is_volume_unit() {
        assert!(is_volume_unit(Some("cup")));
        assert!(is_volume_unit(Some("tbsp")));
        assert!(is_volume_unit(Some("tsp")));
        assert!(!is_volume_unit(Some("oz")));
        assert!(!is_volume_unit(Some("lb")));
        assert!(!is_volume_unit(Some("g")));
        assert!(!is_volume_unit(None));
    }
}
