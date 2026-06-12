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
struct IngredientRule {
    keyword: String,
    keyword_tokens: Vec<String>,
    category: String,
}

static INGREDIENT_MAP: LazyLock<Vec<IngredientRule>> = LazyLock::new(|| {
    let json = include_str!("../../data/ingredients.json");
    let data: IngredientsData =
        serde_json::from_str(json).expect("Failed to parse ingredients.json");

    let mut map: Vec<IngredientRule> = data
        .categories
        .into_iter()
        .map(|(keyword, category)| IngredientRule {
            keyword_tokens: word_tokens(&keyword)
                .into_iter()
                .map(str::to_string)
                .collect(),
            keyword,
            category,
        })
        .collect();
    // Sort by keyword length descending so longer/more specific matches are tried first.
    // Secondary sort by keyword alphabetically for deterministic ordering.
    map.sort_by(|a, b| {
        b.keyword
            .len()
            .cmp(&a.keyword.len())
            .then_with(|| a.keyword.cmp(&b.keyword))
    });
    map
});

/// The canonical set of grocery-aisle categories the categorizer can return.
/// `categorize` always returns one of these (defaulting to "Other").
pub const CATEGORIES: [&str; 19] = [
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
    "Household",
    "Personal Care",
    "Pet",
    "Other",
];

/// Map a category string to its canonical `&'static str`, or "Other" if unknown.
fn category_to_static(category: &str) -> &'static str {
    CATEGORIES
        .into_iter()
        .find(|&c| c == category)
        .unwrap_or("Other")
}

/// Categorize an ingredient by name.
///
/// Returns the category name, or "Other" if no match is found.
/// Matching is case-insensitive and looks for keyword containment.
pub fn categorize(item: &str) -> &'static str {
    let lower = item.to_lowercase();
    let item_tokens = word_tokens(&lower);

    for rule in INGREDIENT_MAP.iter() {
        if keyword_matches(&item_tokens, &rule.keyword_tokens) {
            return category_to_static(&rule.category);
        }
    }

    "Other"
}

fn keyword_matches(item_tokens: &[&str], keyword_tokens: &[String]) -> bool {
    if keyword_tokens.is_empty() || item_tokens.len() < keyword_tokens.len() {
        return false;
    }

    item_tokens
        .windows(keyword_tokens.len())
        .any(|window| keyword_tokens_match(window, keyword_tokens))
}

fn word_tokens(text: &str) -> Vec<&str> {
    text.split(|c| !is_word_char(c))
        .filter(|token| !token.is_empty())
        .collect()
}

fn keyword_tokens_match(item_tokens: &[&str], keyword_tokens: &[String]) -> bool {
    let last_index = keyword_tokens.len() - 1;

    item_tokens.iter().zip(keyword_tokens).enumerate().all(
        |(index, (item_token, keyword_token))| {
            let item_token = *item_token;
            let keyword_token = keyword_token.as_str();
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

    #[test]
    fn test_cookie_snacks() {
        assert_eq!(categorize("Oreo cookie crumbs"), "Snacks");
        assert_eq!(categorize("cookie crumbs"), "Snacks");
        assert_eq!(categorize("cookies"), "Snacks");
    }

    #[test]
    fn test_ginger_mixer_categories() {
        assert_eq!(categorize("ginger beer"), "Beverages");
        assert_eq!(categorize("ginger ale"), "Beverages");
        assert_eq!(categorize("fresh ginger root"), "Produce");
    }

    #[test]
    fn test_chocolate_forms() {
        // Bare "chocolate" on a shopping list is candy; qualified baking forms
        // and chip/chunk/morsel formats are baking aisle.
        assert_eq!(categorize("chocolate"), "Snacks");
        assert_eq!(categorize("bittersweet chocolate, chopped"), "Baking");
        assert_eq!(categorize("semisweet chocolate"), "Baking");
        assert_eq!(categorize("white chocolate"), "Baking");
        assert_eq!(categorize("chocolate chips"), "Baking");
        assert_eq!(categorize("chocolate morsels"), "Baking");
        assert_eq!(categorize("hot chocolate"), "Beverages");
        assert_eq!(categorize("chocolate milk"), "Dairy & Eggs");
        // Bare "chocolate" must not hijack longer non-candy forms.
        assert_eq!(categorize("German Chocolate cake mix"), "Baking");
        assert_eq!(categorize("chocolate frosting"), "Baking");
        assert_eq!(categorize("chocolate ice cream"), "Frozen");
        assert_eq!(categorize("chocolate sauce"), "Condiments & Sauces");
    }

    #[test]
    fn test_water_and_fresh_citrus_juice() {
        assert_eq!(categorize("warm water"), "Beverages");
        // Recipe lemon/lime juice means fresh fruit, not the juice aisle.
        assert_eq!(categorize("freshly squeezed lemon juice"), "Produce");
        assert_eq!(categorize("lime juice from 1 lime"), "Produce");
        assert_eq!(categorize("juice of 1/2 lemon"), "Produce");
        assert_eq!(categorize("apple juice"), "Beverages");
        // "water" must not hijack items that merely mention it.
        assert_eq!(
            categorize("egg wash: 1 egg beaten with water"),
            "Dairy & Eggs"
        );
        assert_eq!(categorize("tuna in water"), "Meat & Seafood");
    }

    #[test]
    fn test_frozen_collisions() {
        // Longer produce keywords used to beat "frozen".
        assert_eq!(categorize("frozen strawberries"), "Frozen");
        assert_eq!(categorize("strawberries"), "Produce");
        assert_eq!(categorize("french fries"), "Frozen");
        assert_eq!(categorize("tater tots"), "Frozen");
    }

    #[test]
    fn test_snack_forms_beat_base_ingredient() {
        assert_eq!(categorize("potato chips"), "Snacks");
        assert_eq!(categorize("potatoes"), "Produce");
        assert_eq!(categorize("tortilla chips"), "Snacks");
        assert_eq!(categorize("tortillas"), "Bakery & Bread");
        assert_eq!(categorize("pretzel twists"), "Snacks");
        assert_eq!(categorize("pretzel buns"), "Bakery & Bread");
    }

    #[test]
    fn test_cereal_brands() {
        assert_eq!(categorize("Rice Chex"), "Snacks");
        assert_eq!(categorize("Lucky Charms"), "Snacks");
        assert_eq!(categorize("M&Ms"), "Snacks");
        assert_eq!(categorize("Rice Krispies"), "Snacks");
        assert_eq!(categorize("rice"), "Pasta & Rice");
    }

    #[test]
    fn test_chiles() {
        assert_eq!(categorize("fresh red chiles, thinly sliced"), "Produce");
        assert_eq!(categorize("ancho chile powder"), "Spices & Seasonings");
        assert_eq!(categorize("chili beans"), "Canned Goods");
        assert_eq!(categorize("chili oil"), "Oils & Vinegars");
        assert_eq!(categorize("Thai sweet chilli sauce"), "Condiments & Sauces");
    }

    #[test]
    fn test_fermented_condiments() {
        assert_eq!(categorize("sauerkraut"), "Condiments & Sauces");
        assert_eq!(categorize("kimchi"), "Condiments & Sauces");
        assert_eq!(categorize("pickles"), "Condiments & Sauces");
        assert_eq!(categorize("pickled ginger"), "Condiments & Sauces");
    }

    #[test]
    fn test_vegetable_option_lists() {
        // Plural "vegetables" means produce; singular "vegetable ..." is
        // usually the first option in an oil/broth list and must not win.
        assert_eq!(categorize("mixed vegetables"), "Produce");
        assert_eq!(categorize("steamed vegetables"), "Produce");
        assert_eq!(
            categorize("vegetable or other neutral-flavored oil"),
            "Oils & Vinegars"
        );
        assert_eq!(
            categorize("vegetable, chicken, or turkey broth"),
            "Canned Goods"
        );
        assert_eq!(categorize("vegetable or chicken stock"), "Canned Goods");
        assert_eq!(categorize("vegetable oil"), "Oils & Vinegars");
        assert_eq!(categorize("vegetable broth"), "Canned Goods");
    }

    #[test]
    fn test_household() {
        assert_eq!(categorize("Dish soap"), "Household");
        assert_eq!(categorize("Palmolive ultra strength"), "Household");
        assert_eq!(categorize("Dishwasher detergent"), "Household");
        assert_eq!(categorize("Cascade Platinum"), "Household");
        assert_eq!(categorize("paper towels"), "Household");
        assert_eq!(categorize("toilet paper"), "Household");
        assert_eq!(categorize("trash bags"), "Household");
        assert_eq!(categorize("aluminum foil"), "Household");
    }

    #[test]
    fn test_personal_care() {
        assert_eq!(categorize("Ibuprofen"), "Personal Care");
        assert_eq!(categorize("Tylenol"), "Personal Care");
        assert_eq!(categorize("toothpaste"), "Personal Care");
        assert_eq!(categorize("shampoo"), "Personal Care");
    }

    #[test]
    fn test_pet() {
        assert_eq!(categorize("Dog treats"), "Pet");
        assert_eq!(categorize("dog food"), "Pet");
        assert_eq!(categorize("cat food"), "Pet");
        assert_eq!(categorize("cat litter"), "Pet");
        // Bare "dog"/"cat" are not keywords; hot dogs stay food.
        assert_eq!(categorize("hot dogs"), "Meat & Seafood");
    }

    #[test]
    fn test_brand_collisions() {
        // Campari the aperitif vs Campari tomatoes.
        assert_eq!(categorize("Campari"), "Beverages");
        assert_eq!(categorize("Campari tomatoes"), "Produce");
        // Scotch the whisky vs scotch bonnet peppers.
        assert_eq!(categorize("scotch"), "Beverages");
        assert_eq!(categorize("scotch bonnet chiles"), "Produce");
        // Fruit the produce vs canning/juice forms.
        assert_eq!(categorize("fresh fruit"), "Produce");
        assert_eq!(categorize("Sure-Jell fruit pectin"), "Baking");
        assert_eq!(categorize("fruit juice"), "Beverages");
    }
}
