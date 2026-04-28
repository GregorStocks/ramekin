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
        if keyword_matches(&lower, keyword) {
            return category_to_static(category);
        }
    }

    "Other"
}

fn keyword_matches(item: &str, keyword: &str) -> bool {
    let item_tokens = word_tokens(item);
    let keyword_tokens = word_tokens(keyword);

    if keyword_tokens.is_empty() || item_tokens.len() < keyword_tokens.len() {
        return false;
    }

    item_tokens
        .windows(keyword_tokens.len())
        .any(|window| keyword_tokens_match(window, &keyword_tokens))
}

fn word_tokens(text: &str) -> Vec<&str> {
    text.split(|c| !is_word_char(c))
        .filter(|token| !token.is_empty())
        .collect()
}

fn keyword_tokens_match(item_tokens: &[&str], keyword_tokens: &[&str]) -> bool {
    let last_index = keyword_tokens.len() - 1;

    item_tokens.iter().zip(keyword_tokens).enumerate().all(
        |(index, (item_token, keyword_token))| {
            if index == last_index {
                token_matches_keyword(item_token, keyword_token)
            } else {
                item_token == keyword_token
            }
        },
    )
}

fn token_matches_keyword(item_token: &str, keyword_token: &str) -> bool {
    item_token == keyword_token
        || item_token
            .strip_prefix(keyword_token)
            .is_some_and(is_allowed_inflection_suffix)
}

fn is_allowed_inflection_suffix(suffix: &str) -> bool {
    matches!(suffix, "s" | "es" | "ies" | "y")
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
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

    #[test]
    fn test_spirits_with_brand_guidance() {
        assert_eq!(
            categorize("London dry gin, such as Beefeater or Tanqueray"),
            "Beverages"
        );
        assert_eq!(
            categorize("rye whiskey, preferably Rittenhouse"),
            "Beverages"
        );
        assert_eq!(
            categorize("dark aged rum, preferably El Dorado 8"),
            "Beverages"
        );
    }

    #[test]
    fn test_keywords_do_not_match_inside_words() {
        assert_eq!(categorize("Beefeater"), "Other");
        assert_eq!(categorize("ginger"), "Produce");
        assert_eq!(categorize("graham cracker crumbs"), "Snacks");
    }
}
