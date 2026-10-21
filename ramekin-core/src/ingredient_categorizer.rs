//! Ingredient categorization for shopping list grouping.
//!
//! Maps ingredient names to grocery store aisle categories: the ingredient
//! catalog's category for the food a name resolves to, then a learned answer's,
//! then the aisle Open Food Facts gives the name.

/// The canonical set of grocery-aisle categories the categorizer can return.
/// `categorize` always returns one of these (defaulting to "Other").
pub const CATEGORIES: [&str; 18] = [
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
    "Personal Care",
    "Pet",
    "Other",
];

/// Categorize an ingredient by name, with no learned answers.
pub fn categorize(item: &str) -> &'static str {
    categorize_with(item, &crate::catalog::Learned::new())
}

/// The category for `item`: "Frozen" for a name starting with "frozen", else
/// the committed catalog's category (the food a name resolves to, a product
/// such as parchment paper, an ambiguous name such as "cheese", or the first
/// food of a "salt and pepper" line), else that of the learned entry, else the
/// aisle the model gave a learned estimate ("harissa"), else the aisle Open
/// Food Facts gives the name ("cheerios", "cheez it"). A raw line with an
/// amount or a note ("1 medium white onion", "furikake (see note 3)"), as a
/// shopping list can hold, is read as an ingredient line when its text alone
/// gets no category. Anything else is "Other".
pub fn categorize_with(item: &str, learned: &crate::catalog::Learned) -> &'static str {
    let parsed = parsed_name(item);
    committed_category(item, parsed.as_deref(), learned)
        .or_else(|| open_food_facts_category(item, parsed.as_deref()))
        .unwrap_or("Other")
}

/// The item a raw line ("1 medium white onion") parses to, if it differs.
fn parsed_name(item: &str) -> Option<String> {
    use crate::catalog::normalize;
    crate::ingredient_parser::reparse_item(item)
        .map(|parsed| parsed.item)
        .filter(|parsed| normalize(parsed) != normalize(item))
}

/// "Frozen" for a name starting with "frozen", else the committed catalog's
/// category for the item or its parsed name, else that of the learned entry,
/// else a learned estimate's aisle.
fn committed_category(
    item: &str,
    parsed: Option<&str>,
    learned: &crate::catalog::Learned,
) -> Option<&'static str> {
    use crate::catalog::{normalize, LearnedTarget};
    // The catalog drops "frozen" to find the food ("2 bags frozen peaches"
    // are peaches), but the shopper still buys them in the freezer aisle.
    let frozen = |name: &str| normalize(name).starts_with("frozen ");
    if frozen(item) || parsed.is_some_and(frozen) {
        return Some("Frozen");
    }
    let lookup = |name: &str| {
        crate::catalog::category(name).or_else(|| match learned_target(name, learned)? {
            LearnedTarget::Entry(key) => crate::catalog::category(key),
            LearnedTarget::Estimate(estimate) => estimate.category,
            LearnedTarget::NotFood | LearnedTarget::Unknown => None,
        })
    };
    std::iter::once(item).chain(parsed).find_map(lookup)
}

/// The aisle Open Food Facts gives the item, or else its parsed name.
fn open_food_facts_category(item: &str, parsed: Option<&str>) -> Option<&'static str> {
    std::iter::once(item)
        .chain(parsed)
        .find_map(crate::catalog::open_food_facts_category)
}

/// The stored answer for `item`, if the catalog doesn't know the name itself.
fn learned_target<'a>(
    item: &str,
    learned: &'a crate::catalog::Learned,
) -> Option<&'a crate::catalog::LearnedTarget> {
    use crate::catalog::{normalize, unlearned_name};
    unlearned_name(item).and_then(|name| learned.get(&normalize(&name)))
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
    fn test_catalog_reads_a_raw_line() {
        // Shopping lists hold raw lines; the catalog finds the food once the
        // amount or note is split off.
        let learned = crate::catalog::Learned::new();
        assert_eq!(categorize_with("1 medium white onion", &learned), "Produce");
        assert_eq!(
            categorize_with("furikake (see note 3)", &learned),
            "Spices & Seasonings"
        );
        assert_eq!(categorize_with("2 bags frozen peaches", &learned), "Frozen");
        assert_eq!(categorize_with("xyzfoobar123", &learned), "Other");
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
    fn test_names_inside_other_words() {
        assert_eq!(categorize("Beefeater"), "Other");
        assert_eq!(categorize("ginger"), "Produce");
        assert_eq!(categorize("graham cracker crumbs"), "Snacks");
    }

    #[test]
    fn test_cookie_snacks() {
        assert_eq!(categorize("Oreo cookie crumbs"), "Snacks");
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
        // and chips are baking aisle.
        assert_eq!(categorize("chocolate"), "Snacks");
        assert_eq!(categorize("bittersweet chocolate, chopped"), "Baking");
        assert_eq!(categorize("semisweet chocolate"), "Baking");
        assert_eq!(categorize("white chocolate"), "Baking");
        assert_eq!(categorize("chocolate chips"), "Baking");
        assert_eq!(categorize("hot chocolate"), "Beverages");
        assert_eq!(categorize("chocolate milk"), "Dairy & Eggs");
        // Bare "chocolate" must not hijack longer non-candy forms.
        assert_eq!(categorize("German Chocolate cake mix"), "Baking");
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
        // Items that merely mention water aren't water.
        assert_eq!(
            categorize("egg wash: 1 egg beaten with water"),
            "Dairy & Eggs"
        );
        assert_eq!(categorize("tuna in water"), "Meat & Seafood");
    }

    #[test]
    fn test_frozen_collisions() {
        assert_eq!(categorize("frozen strawberries"), "Frozen");
        assert_eq!(categorize("strawberries"), "Produce");
        assert_eq!(categorize("french fries"), "Frozen");
    }

    #[test]
    fn test_frozen_beats_the_catalog_food() {
        // The catalog resolves these to the fresh fruit.
        assert_eq!(categorize("frozen peaches"), "Frozen");
        assert_eq!(categorize("Frozen mango chunks"), "Frozen");
        assert_eq!(categorize("peaches"), "Produce");
        // The first alternative decides.
        assert_eq!(categorize("fresh or frozen corn kernels"), "Produce");
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
        assert_eq!(categorize("chili beans"), "Canned Goods");
        assert_eq!(categorize("chili oil"), "Oils & Vinegars");
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
    fn test_household_goods_are_other() {
        // A cooking app has no aisle for these.
        assert_eq!(categorize("Dish soap"), "Other");
        assert_eq!(categorize("Palmolive ultra strength"), "Other");
        assert_eq!(categorize("Dishwasher detergent"), "Other");
        assert_eq!(categorize("Cascade Platinum"), "Other");
        assert_eq!(categorize("paper towels"), "Other");
        assert_eq!(categorize("toilet paper"), "Other");
        assert_eq!(categorize("trash bags"), "Other");
        assert_eq!(categorize("aluminum foil"), "Other");
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
        // Hot dogs stay food.
        assert_eq!(categorize("hot dogs"), "Meat & Seafood");
    }

    #[test]
    fn test_fruit_forms() {
        assert_eq!(categorize("fresh fruit"), "Produce");
        assert_eq!(categorize("fruit juice"), "Beverages");
    }

    #[test]
    fn test_open_food_facts_names() {
        let learned = crate::catalog::Learned::new();
        // Brands and product names the catalog doesn't know.
        assert_eq!(categorize_with("Cheerios", &learned), "Snacks");
        assert_eq!(categorize_with("Cheez-Its", &learned), "Snacks");
        assert_eq!(categorize_with("Baked beans", &learned), "Canned Goods");
        // The catalog still wins: Open Food Facts files basil with the spices.
        assert_eq!(
            crate::catalog::open_food_facts_category("basil"),
            Some("Spices & Seasonings")
        );
        assert_eq!(categorize("basil"), "Produce");
        // Its taxonomy files "spelt flour" as the grain; the catalog knows the
        // flour.
        assert_eq!(
            crate::catalog::open_food_facts_category("spelt flour"),
            Some("Pasta & Rice")
        );
        assert_eq!(categorize("spelt flour"), "Baking");
    }

    #[test]
    fn test_products_and_compound_lines() {
        assert_eq!(categorize("Parchment paper"), "Other");
        assert_eq!(
            categorize("wooden skewers, soaked in cold water for 30 minutes"),
            "Other"
        );
        // A compound line takes its first food's category.
        assert_eq!(categorize("salt and pepper"), "Spices & Seasonings");
    }
}
