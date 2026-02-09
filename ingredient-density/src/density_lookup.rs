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
    /// Per-domain overrides: domain -> ingredient name -> canonical name.
    /// Used when a site has a known convention (e.g., "salt" means Diamond Crystal on smittenkitchen).
    #[serde(default)]
    domain_overrides: HashMap<String, HashMap<String, String>>,
}

/// Merged density data from all sources.
struct MergedData {
    /// Ingredient name -> grams per cup
    ingredients: HashMap<String, f64>,
    /// Alias -> canonical name (or None if explicitly ambiguous)
    aliases: HashMap<String, Option<String>>,
    /// Per-domain overrides: domain -> ingredient name -> canonical name
    domain_overrides: HashMap<String, HashMap<String, String>>,
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

    // Normalize domain override keys to lowercase
    let domain_overrides = curated
        .domain_overrides
        .into_iter()
        .map(|(domain, overrides)| {
            let normalized: HashMap<String, String> = overrides
                .into_iter()
                .map(|(k, v)| (k.to_lowercase(), v))
                .collect();
            (domain, normalized)
        })
        .collect();

    MergedData {
        ingredients,
        aliases,
        domain_overrides,
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
/// When `domain` is provided, domain-specific overrides are checked first,
/// allowing per-site salt conventions (e.g., "salt" means Diamond Crystal
/// on smittenkitchen.com but table salt on allrecipes.com).
///
/// Lookup order:
/// 1. Domain override (if domain provided and ingredient has an override)
/// 2. Direct lookup in ingredients
/// 3. Lookup via aliases (returns None if alias is explicitly null/ambiguous)
/// 4. Try plural/singular variations
/// 5. After stripping common modifiers, retry steps 2-4
pub fn find_density(ingredient_item: &str, domain: Option<&str>) -> Option<f64> {
    let normalized = normalize_ingredient_name(ingredient_item);

    // Check domain overrides first
    if let Some(domain) = domain {
        if let Some(overrides) = DATA.domain_overrides.get(domain) {
            if let Some(canonical) = overrides.get(&normalized) {
                if let Some(&density) = DATA.ingredients.get(canonical) {
                    return Some(density);
                }
            }
        }
    }

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
        assert!(find_density("salt, table", None).is_some());
    }

    #[test]
    fn test_find_density_alias() {
        // Common aliases should work
        assert!(find_density("flour", None).is_some());
        assert!(find_density("sugar", None).is_some());
        assert!(find_density("butter", None).is_some());
    }

    #[test]
    fn test_find_density_with_modifiers() {
        assert!(find_density("softened butter", None).is_some());
        assert!(find_density("melted butter", None).is_some());
    }

    #[test]
    fn test_find_density_case_insensitive() {
        assert!(find_density("FLOUR", None).is_some());
        assert!(find_density("Butter", None).is_some());
    }

    #[test]
    fn test_find_density_unknown() {
        assert_eq!(find_density("unicorn tears", None), None);
        assert_eq!(find_density("mystery powder", None), None);
    }

    #[test]
    fn test_plural_fallback() {
        // "onion" should find "onions" (USDA has plural)
        assert!(find_density("onion", None).is_some());
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
        assert!(find_density("baking powder", None).is_some());
        assert!(find_density("baking soda", None).is_some());
        // Spices
        assert!(find_density("cinnamon", None).is_some());
        assert!(find_density("ground cinnamon", None).is_some());
        assert!(find_density("garlic powder", None).is_some());
        // Onion varieties
        assert!(find_density("yellow onion", None).is_some());
        assert!(find_density("red onion", None).is_some());
        // Dairy
        assert!(find_density("buttermilk", None).is_some());
        assert!(find_density("greek yogurt", None).is_some());
        // Condiments
        assert!(find_density("mayo", None).is_some());
        assert!(find_density("mayonnaise", None).is_some());
        assert!(find_density("mustard", None).is_some());
        assert!(find_density("yellow mustard", None).is_some());
        assert!(find_density("dijon mustard", None).is_some());
        // Other
        assert!(find_density("water", None).is_some());
    }

    #[test]
    fn test_ambiguous_aliases_return_none() {
        // Salt varieties are globally ambiguous (different densities by site/brand)
        assert!(find_density("salt", None).is_none());
        assert!(find_density("kosher salt", None).is_none());
        assert!(find_density("sea salt", None).is_none());
        // Pepper is ambiguous
        assert!(find_density("black pepper", None).is_none());
        assert!(find_density("ground black pepper", None).is_none());
    }

    #[test]
    fn test_fine_salt_aliases() {
        // Fine salt variants are unambiguous (~292 g/cup, same as table salt)
        assert!(find_density("fine salt", None).is_some());
        assert!(find_density("fine sea salt", None).is_some());
        assert!(find_density("fine grain sea salt", None).is_some());
        assert!(find_density("fine-grain sea salt", None).is_some());
        assert!(find_density("fine sea or table salt", None).is_some());
        assert!(find_density("fine grain salt", None).is_some());
    }

    #[test]
    fn test_domain_overrides() {
        // Without domain, salt is ambiguous
        assert!(find_density("salt", None).is_none());
        assert!(find_density("kosher salt", None).is_none());

        // smittenkitchen uses Diamond Crystal kosher salt
        let sk = Some("smittenkitchen.com");
        let salt_density =
            find_density("salt", sk).expect("salt should resolve for smittenkitchen");
        assert!((salt_density - 137.0).abs() < 0.1);
        let kosher_density =
            find_density("kosher salt", sk).expect("kosher salt should resolve for smittenkitchen");
        assert!((kosher_density - 137.0).abs() < 0.1);

        // allrecipes uses table salt
        let ar = Some("allrecipes.com");
        let salt_density = find_density("salt", ar).expect("salt should resolve for allrecipes");
        assert!((salt_density - 292.0).abs() < 0.1);
        // kosher salt has no override on allrecipes, stays ambiguous
        assert!(find_density("kosher salt", ar).is_none());

        // Unknown domain gets no override
        assert!(find_density("salt", Some("unknownsite.com")).is_none());
    }

    #[test]
    fn test_top_density_aliases() {
        // Top 10 from density gap report
        assert!(find_density("soy sauce", None).is_some());
        assert!(find_density("ground cumin", None).is_some());
        assert!(find_density("dried oregano", None).is_some());
        assert!(find_density("pure vanilla extract", None).is_some());
        assert!(find_density("Worcestershire sauce", None).is_some());
        assert!(find_density("fresh lemon juice", None).is_some());
        assert!(find_density("tomato paste", None).is_some());
        assert!(find_density("sesame oil", None).is_some());
        assert!(find_density("vanilla", None).is_some());
        assert!(find_density("rice vinegar", None).is_some());
        // Related aliases
        assert!(find_density("cumin", None).is_some());
        assert!(find_density("oregano", None).is_some());
        assert!(find_density("toasted sesame oil", None).is_some());
        assert!(find_density("apple cider vinegar", None).is_some());
        assert!(find_density("red wine vinegar", None).is_some());
        assert!(find_density("balsamic vinegar", None).is_some());
        assert!(find_density("white vinegar", None).is_some());
        assert!(find_density("white wine vinegar", None).is_some());
        assert!(find_density("tamari", None).is_some());
        assert!(find_density("Japanese soy sauce (koikuchi shoyu)", None).is_some());
    }
}
