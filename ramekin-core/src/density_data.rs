//! Ingredient density data for volume-to-weight conversion.
//!
//! Densities are stored as grams per US cup (236.588 ml).
//! Data curated from reliable sources (King Arthur Baking, USDA).

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

/// Ingredient density data: canonical name -> grams per cup.
pub static DENSITY_DATA: LazyLock<HashMap<&'static str, f64>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Flours
    m.insert("all-purpose flour", 125.0);
    m.insert("bread flour", 127.0);
    m.insert("cake flour", 114.0);
    m.insert("whole wheat flour", 120.0);
    m.insert("almond flour", 96.0);
    m.insert("coconut flour", 112.0);

    // Sugars
    m.insert("granulated sugar", 200.0);
    m.insert("brown sugar", 220.0); // packed
    m.insert("powdered sugar", 120.0);
    m.insert("honey", 340.0);
    m.insert("maple syrup", 315.0);

    // Dairy
    m.insert("butter", 227.0);
    m.insert("milk", 245.0);
    m.insert("heavy cream", 238.0);
    m.insert("sour cream", 242.0);
    m.insert("cream cheese", 232.0);

    // Fats/Oils
    m.insert("vegetable oil", 218.0);
    m.insert("olive oil", 216.0);
    m.insert("coconut oil", 218.0);

    // Other common
    m.insert("rolled oats", 80.0);
    m.insert("cornstarch", 128.0);
    m.insert("cocoa powder", 86.0);
    m.insert("peanut butter", 258.0);

    m
});

/// Aliases mapping common ingredient names to canonical names in DENSITY_DATA.
pub static INGREDIENT_ALIASES: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        let mut m = HashMap::new();

        // Flour aliases
        m.insert("flour", "all-purpose flour");
        m.insert("ap flour", "all-purpose flour");
        m.insert("plain flour", "all-purpose flour");
        m.insert("white flour", "all-purpose flour");

        // Sugar aliases
        m.insert("sugar", "granulated sugar");
        m.insert("white sugar", "granulated sugar");
        m.insert("caster sugar", "granulated sugar");
        m.insert("confectioners sugar", "powdered sugar");
        m.insert("confectioners' sugar", "powdered sugar");
        m.insert("icing sugar", "powdered sugar");
        m.insert("light brown sugar", "brown sugar");
        m.insert("dark brown sugar", "brown sugar");
        m.insert("packed brown sugar", "brown sugar");

        // Butter aliases
        m.insert("unsalted butter", "butter");
        m.insert("salted butter", "butter");

        // Oil aliases
        m.insert("oil", "vegetable oil");
        m.insert("canola oil", "vegetable oil");
        m.insert("extra virgin olive oil", "olive oil");
        m.insert("extra-virgin olive oil", "olive oil");

        // Cream aliases
        m.insert("whipping cream", "heavy cream");
        m.insert("heavy whipping cream", "heavy cream");
        m.insert("double cream", "heavy cream");
        m.insert("whole milk", "milk");

        // Oats aliases
        m.insert("oats", "rolled oats");
        m.insert("old-fashioned oats", "rolled oats");
        m.insert("old fashioned oats", "rolled oats");

        // Other aliases
        m.insert("corn starch", "cornstarch");
        m.insert("unsweetened cocoa powder", "cocoa powder");
        m.insert("dutch process cocoa powder", "cocoa powder");
        m.insert("natural cocoa powder", "cocoa powder");
        m.insert("pure maple syrup", "maple syrup");

        m
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
/// 1. Direct lookup in DENSITY_DATA
/// 2. Lookup via INGREDIENT_ALIASES
/// 3. After stripping common modifiers, retry both lookups
pub fn find_density(ingredient_item: &str) -> Option<f64> {
    let normalized = normalize_ingredient_name(ingredient_item);

    // Direct lookup
    if let Some(&density) = DENSITY_DATA.get(normalized.as_str()) {
        return Some(density);
    }

    // Alias lookup
    if let Some(&canonical) = INGREDIENT_ALIASES.get(normalized.as_str()) {
        if let Some(&density) = DENSITY_DATA.get(canonical) {
            return Some(density);
        }
    }

    // Try with modifiers stripped
    let stripped = strip_modifiers(&normalized);
    if stripped != normalized {
        if let Some(&density) = DENSITY_DATA.get(stripped.as_str()) {
            return Some(density);
        }
        if let Some(&canonical) = INGREDIENT_ALIASES.get(stripped.as_str()) {
            if let Some(&density) = DENSITY_DATA.get(canonical) {
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
        assert_eq!(find_density("all-purpose flour"), Some(125.0));
        assert_eq!(find_density("butter"), Some(227.0));
        assert_eq!(find_density("granulated sugar"), Some(200.0));
    }

    #[test]
    fn test_find_density_alias() {
        assert_eq!(find_density("flour"), Some(125.0));
        assert_eq!(find_density("sugar"), Some(200.0));
        assert_eq!(find_density("unsalted butter"), Some(227.0));
    }

    #[test]
    fn test_find_density_with_modifiers() {
        assert_eq!(find_density("softened butter"), Some(227.0));
        assert_eq!(find_density("butter, softened"), Some(227.0));
        assert_eq!(find_density("room temperature butter"), Some(227.0));
        assert_eq!(find_density("melted butter"), Some(227.0));
    }

    #[test]
    fn test_find_density_case_insensitive() {
        assert_eq!(find_density("ALL-PURPOSE FLOUR"), Some(125.0));
        assert_eq!(find_density("Butter"), Some(227.0));
        assert_eq!(find_density("SUGAR"), Some(200.0));
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
