//! Ingredient categorization for shopping list grouping.
//!
//! Maps ingredient names to grocery store aisle categories based on keyword matching.
//! Category data is loaded from `data/ingredients.json` at compile time.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

/// The raw JSON structure for ingredients data file.
#[derive(Deserialize)]
struct IngredientsData {
    categories: HashMap<String, String>,
}

/// Ingredient map loaded from JSON and sorted by keyword length (longest first).
/// This ensures more specific matches are tried before general ones.
static INGREDIENT_MAP: LazyLock<Vec<(String, String)>> = LazyLock::new(|| {
    let json = include_str!("../../data/ingredients.json");
    let data: IngredientsData =
        serde_json::from_str(json).expect("Failed to parse ingredients.json");

    let mut map: Vec<(String, String)> = data.categories.into_iter().collect();
    // Sort by keyword length descending so longer/more specific matches are tried first.
    // Secondary sort by keyword alphabetically for deterministic ordering.
    map.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
    map
});

/// Convert a category string to a static str.
/// We cache the static strings to avoid allocation on every call.
fn category_to_static(category: &str) -> &'static str {
    static STATIC_CATEGORIES: LazyLock<HashMap<String, &'static str>> = LazyLock::new(|| {
        let categories = [
            "Produce",
            "Meat & Seafood",
            "Dairy & Eggs",
            "Cheese",
            "Bakery & Bread",
            "Frozen",
            "Pasta & Rice",
            "Canned Goods",
            "Baking",
            "Spices & Seasonings",
            "Condiments & Sauces",
            "Oils & Vinegars",
            "Nuts & Dried Fruit",
            "Beverages",
            "Snacks",
            "Other",
        ];
        categories.iter().map(|&c| (c.to_string(), c)).collect()
    });

    STATIC_CATEGORIES.get(category).copied().unwrap_or("Other")
}

/// Categorize an ingredient by name.
///
/// Returns the category name, or "Other" if no match is found.
/// Matching is case-insensitive and looks for keyword containment.
pub fn categorize(item: &str) -> &'static str {
    let lower = item.to_lowercase();

    for (keyword, category) in INGREDIENT_MAP.iter() {
        if lower.contains(keyword) {
            return category_to_static(category);
        }
    }

    "Other"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_produce() {
        assert_eq!(categorize("chicken breast"), "Meat & Seafood");
        assert_eq!(categorize("Olive Oil"), "Oils & Vinegars");
        assert_eq!(categorize("tomatoes"), "Produce");
        assert_eq!(categorize("Fresh Basil"), "Produce");
        assert_eq!(categorize("dried basil"), "Spices & Seasonings");
    }

    #[test]
    fn test_dairy() {
        assert_eq!(categorize("butter"), "Dairy & Eggs");
        assert_eq!(categorize("eggs"), "Dairy & Eggs");
        assert_eq!(categorize("Greek Yogurt"), "Dairy & Eggs");
    }

    #[test]
    fn test_cheese() {
        assert_eq!(categorize("parmesan cheese"), "Cheese");
        assert_eq!(categorize("mozzarella"), "Cheese");
        assert_eq!(categorize("cream cheese"), "Cheese");
    }

    #[test]
    fn test_unknown() {
        assert_eq!(categorize("xyzfoobar123"), "Other");
        assert_eq!(categorize(""), "Other");
    }
}
