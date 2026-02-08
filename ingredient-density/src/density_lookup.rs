//! Ingredient density lookup for volume-to-weight conversion.
//!
//! Densities are stored as grams per US cup (236.588 ml).
//! Data sourced from USDA FoodData Central (public domain, CC0)
//! with optional curated overrides.

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

// =============================================================================
// Data structures
// =============================================================================

/// USDA data format (simple name -> density mapping).
#[derive(Deserialize)]
struct UsdaDataFile {
    ingredients: HashMap<String, f64>,
    aliases: HashMap<String, String>,
}

/// Curated ingredient entry with citation.
#[derive(Deserialize)]
struct CuratedIngredient {
    grams_per_cup: f64,
    #[allow(dead_code)]
    source: String,
    #[allow(dead_code)]
    url: Option<String>,
}

/// Curated data format (with citations and nullable aliases).
#[derive(Deserialize)]
struct CuratedDataFile {
    ingredients: HashMap<String, CuratedIngredient>,
    /// Aliases can be null to indicate "explicitly ambiguous, do not resolve"
    aliases: HashMap<String, Option<String>>,
}

/// Merged density data from all sources.
struct MergedData {
    /// Ingredient name -> grams per cup
    ingredients: HashMap<String, f64>,
    /// Alias -> canonical name (or None if explicitly ambiguous)
    aliases: HashMap<String, Option<String>>,
}

// =============================================================================
// Data loading
// =============================================================================

/// Embedded JSON data files.
static USDA_JSON: &str = include_str!("data/usda.json");
static CURATED_JSON: &str = include_str!("data/curated.json");

/// Parsed and merged density data.
static DATA: LazyLock<MergedData> = LazyLock::new(|| {
    let usda: UsdaDataFile =
        serde_json::from_str(USDA_JSON).expect("usda.json should be valid JSON");
    let curated: CuratedDataFile =
        serde_json::from_str(CURATED_JSON).expect("curated.json should be valid JSON");

    // Start with USDA data
    let mut ingredients = usda.ingredients;
    let mut aliases: HashMap<String, Option<String>> = usda
        .aliases
        .into_iter()
        .map(|(k, v)| (k, Some(v)))
        .collect();

    // Override with curated data (curated takes precedence)
    for (name, entry) in curated.ingredients {
        ingredients.insert(name, entry.grams_per_cup);
    }
    for (alias, canonical) in curated.aliases {
        aliases.insert(alias, canonical);
    }

    MergedData {
        ingredients,
        aliases,
    }
});

// =============================================================================
// Modifier stripping
// =============================================================================

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

// =============================================================================
// Plural handling
// =============================================================================

/// Try plural/singular variations of a name.
/// Returns the density if found via plural variation.
fn try_plural_variations(name: &str, ingredients: &HashMap<String, f64>) -> Option<f64> {
    // Try adding 's' for singular -> plural (e.g., "onion" -> "onions")
    let with_s = format!("{name}s");
    if let Some(&density) = ingredients.get(&with_s) {
        return Some(density);
    }

    // Try removing 's' for plural -> singular (e.g., "eggs" -> "egg")
    if let Some(without_s) = name.strip_suffix('s') {
        if let Some(&density) = ingredients.get(without_s) {
            return Some(density);
        }
    }

    None
}

// =============================================================================
// Public API
// =============================================================================

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

/// Normalize ingredient name for matching.
fn normalize_ingredient_name(s: &str) -> String {
    s.to_lowercase().trim().to_string()
}

/// Find the density (grams per cup) for an ingredient name.
///
/// Lookup order:
/// 1. Direct lookup in ingredients
/// 2. Lookup via aliases (returns None if alias is explicitly null/ambiguous)
/// 3. Try plural/singular variations
/// 4. After stripping common modifiers, retry steps 1-3
pub fn find_density(ingredient_item: &str) -> Option<f64> {
    let normalized = normalize_ingredient_name(ingredient_item);

    // Helper to do full lookup chain
    fn lookup(name: &str, data: &MergedData) -> Option<f64> {
        // Direct lookup
        if let Some(&density) = data.ingredients.get(name) {
            return Some(density);
        }

        // Alias lookup
        if let Some(canonical_opt) = data.aliases.get(name) {
            match canonical_opt {
                Some(canonical) => {
                    if let Some(&density) = data.ingredients.get(canonical) {
                        return Some(density);
                    }
                }
                None => {
                    // Explicitly ambiguous alias - return None immediately
                    return None;
                }
            }
        }

        // Plural/singular variations
        if let Some(density) = try_plural_variations(name, &data.ingredients) {
            return Some(density);
        }

        None
    }

    // Try with original normalized name
    if let Some(density) = lookup(&normalized, &DATA) {
        return Some(density);
    }

    // Try with modifiers stripped
    let stripped = strip_modifiers(&normalized);
    if stripped != normalized {
        if let Some(density) = lookup(&stripped, &DATA) {
            return Some(density);
        }
    }

    None
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_density_direct() {
        // USDA has "salt, table"
        assert!(find_density("salt, table").is_some());
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
    fn test_plural_fallback() {
        // "onion" should find "onions" (USDA has plural)
        assert!(find_density("onion").is_some());
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

    #[test]
    fn test_curated_aliases() {
        // Leavening agents
        assert!(find_density("baking powder").is_some());
        assert!(find_density("baking soda").is_some());
        // Spices
        assert!(find_density("cinnamon").is_some());
        assert!(find_density("ground cinnamon").is_some());
        assert!(find_density("garlic powder").is_some());
        // Onion varieties
        assert!(find_density("yellow onion").is_some());
        assert!(find_density("red onion").is_some());
        // Dairy
        assert!(find_density("buttermilk").is_some());
        assert!(find_density("greek yogurt").is_some());
        // Condiments
        assert!(find_density("mayo").is_some());
        assert!(find_density("mayonnaise").is_some());
        assert!(find_density("mustard").is_some());
        assert!(find_density("yellow mustard").is_some());
        assert!(find_density("dijon mustard").is_some());
        // Other
        assert!(find_density("water").is_some());
    }

    #[test]
    fn test_ambiguous_aliases_return_none() {
        // Salt varieties are ambiguous (different densities)
        assert!(find_density("salt").is_none());
        assert!(find_density("kosher salt").is_none());
        assert!(find_density("sea salt").is_none());
        // Pepper is ambiguous
        assert!(find_density("black pepper").is_none());
        assert!(find_density("ground black pepper").is_none());
    }

    #[test]
    fn test_top_density_aliases() {
        // Top 10 from density gap report
        assert!(find_density("soy sauce").is_some());
        assert!(find_density("ground cumin").is_some());
        assert!(find_density("dried oregano").is_some());
        assert!(find_density("pure vanilla extract").is_some());
        assert!(find_density("Worcestershire sauce").is_some());
        assert!(find_density("fresh lemon juice").is_some());
        assert!(find_density("tomato paste").is_some());
        assert!(find_density("sesame oil").is_some());
        assert!(find_density("vanilla").is_some());
        assert!(find_density("rice vinegar").is_some());
        // Related aliases
        assert!(find_density("cumin").is_some());
        assert!(find_density("oregano").is_some());
        assert!(find_density("toasted sesame oil").is_some());
        assert!(find_density("apple cider vinegar").is_some());
        assert!(find_density("red wine vinegar").is_some());
        assert!(find_density("balsamic vinegar").is_some());
        assert!(find_density("white vinegar").is_some());
        assert!(find_density("white wine vinegar").is_some());
        assert!(find_density("tamari").is_some());
        assert!(find_density("Japanese soy sauce (koikuchi shoyu)").is_some());
    }
}
