use super::*;
use crate::ingredient_parser::item::line_classifiers::title_case;
use crate::ingredient_parser::parse_ingredient;

#[test]
fn test_detect_section_header_for_the_pattern() {
    // "For the X" patterns should be normalized
    assert_eq!(
        detect_section_header("For the sauce:"),
        Some("For the Sauce".to_string())
    );
    assert_eq!(
        detect_section_header("For the dough:"),
        Some("For the Dough".to_string())
    );
    assert_eq!(
        detect_section_header("For serving:"),
        Some("For Serving".to_string())
    );
    // Already title case should be preserved
    assert_eq!(
        detect_section_header("For the Steak Fajita Marinade:"),
        Some("For the Steak Fajita Marinade".to_string())
    );
}

#[test]
fn test_detect_section_header_all_caps() {
    // All-caps headers WITH colon should be normalized to title case
    assert_eq!(
        detect_section_header("FILLING:"),
        Some("Filling".to_string())
    );
    assert_eq!(
        detect_section_header("DRIZZLE:"),
        Some("Drizzle".to_string())
    );
    assert_eq!(
        detect_section_header("TOPPING:"),
        Some("Topping".to_string())
    );
    assert_eq!(
        detect_section_header("TOPPINGS, OPTIONAL:"),
        Some("Toppings, Optional".to_string())
    );
    assert_eq!(
        detect_section_header("FOR THE SAUCE:"),
        Some("For the Sauce".to_string())
    );
}

#[test]
fn test_detect_section_header_all_caps_no_colon() {
    // All-caps headers WITHOUT colon should also be detected
    assert_eq!(
        detect_section_header("FILLING"),
        Some("Filling".to_string())
    );
    assert_eq!(detect_section_header("DOUGH"), Some("Dough".to_string()));
    assert_eq!(
        detect_section_header("ASSEMBLY"),
        Some("Assembly".to_string())
    );
    assert_eq!(
        detect_section_header("BERRY SAUCE"),
        Some("Berry Sauce".to_string())
    );
    assert_eq!(
        detect_section_header("WHIPPED COTTAGE CHEESE"),
        Some("Whipped Cottage Cheese".to_string())
    );
    assert_eq!(
        detect_section_header("TOASTS AND ASSEMBLY"),
        Some("Toasts and Assembly".to_string())
    );
}

#[test]
fn test_detect_section_header_ingredients_suffix() {
    // Lines ending with "Ingredients" should be detected
    assert_eq!(
        detect_section_header("Topping Ingredients:"),
        Some("Topping Ingredients".to_string())
    );
    assert_eq!(
        detect_section_header("Crust Ingredients:"),
        Some("Crust Ingredients".to_string())
    );
    assert_eq!(
        detect_section_header("Optional Frosting Ingredients:"),
        Some("Optional Frosting Ingredients".to_string())
    );
}

#[test]
fn test_detect_section_header_mixed_case_keywords() {
    // Mixed-case headers with section keywords should be detected
    assert_eq!(
        detect_section_header("Toppings:"),
        Some("Toppings".to_string())
    );
    assert_eq!(
        detect_section_header("optional toppings:"),
        Some("Optional Toppings".to_string())
    );
    assert_eq!(
        detect_section_header("Cream cheese filling:"),
        Some("Cream Cheese Filling".to_string())
    );
    assert_eq!(
        detect_section_header("Chocolate Glaze:"),
        Some("Chocolate Glaze".to_string())
    );
    assert_eq!(
        detect_section_header("For serving:"),
        Some("For Serving".to_string())
    );
}

#[test]
fn test_detect_section_header_long_keyword_fallback() {
    // Headers longer than five words still count as sections when they
    // contain a well-known section keyword.
    assert!(detect_section_header("Creamy Artichoke Spread (makes a little extra):").is_some());
    assert!(detect_section_header("PART III: The ham and nut filling:").is_some());
    // A long phrase without any recognized keyword stays an ingredient
    // (we don't have enough signal to call it a header).
    assert_eq!(
        detect_section_header("This is a long phrase with no keyword at all:"),
        None
    );
}

#[test]
fn test_detect_section_header_to_assemble_pattern() {
    // Imperative "To X:" headers (parallel to "For X:") should be detected
    assert_eq!(
        detect_section_header("To assemble:"),
        Some("To Assemble".to_string())
    );
    assert_eq!(
        detect_section_header("To serve:"),
        Some("To Serve".to_string())
    );
    assert_eq!(
        detect_section_header("To finish:"),
        Some("To Finish".to_string())
    );
    assert_eq!(
        detect_section_header("To assemble the cake:"),
        Some("To Assemble the Cake".to_string())
    );
}

#[test]
fn test_detect_section_header_single_word() {
    // Single-word headers ending with colon should be detected
    assert_eq!(detect_section_header("Dough:"), Some("Dough".to_string()));
    assert_eq!(detect_section_header("Brine:"), Some("Brine".to_string()));
    assert_eq!(
        detect_section_header("Chicken:"),
        Some("Chicken".to_string())
    );
    assert_eq!(detect_section_header("Eggs:"), Some("Eggs".to_string()));
    assert_eq!(
        detect_section_header("Caramel:"),
        Some("Caramel".to_string())
    );
    assert_eq!(
        detect_section_header("Meatballs:"),
        Some("Meatballs".to_string())
    );
}

#[test]
fn test_detect_section_header_not_header() {
    // Regular ingredients should return None
    assert_eq!(detect_section_header("1 cup flour"), None);
    assert_eq!(detect_section_header("2 tablespoons oil"), None);
    // Ingredient with colon in note should not be detected as header
    assert_eq!(detect_section_header("butter: softened"), None);
}

#[test]
fn test_should_ignore_line() {
    // Scraper artifacts should be ignored
    assert!(should_ignore_line("Gather Your Ingredients"));
    assert!(should_ignore_line("gather your ingredients"));
    assert!(should_ignore_line("GATHER YOUR INGREDIENTS"));
    assert!(should_ignore_line("Special equipment: Spice grinder"));
    assert!(should_ignore_line("Equipment: Stand mixer"));
    assert!(should_ignore_line("Notes: See recipe headnotes"));
    assert!(should_ignore_line("(This makes a 12-ounce batch.)"));
    assert!(should_ignore_line("Yield: 10 3-inch biscuits"));
    assert!(should_ignore_line("Yield 26 to 28 cookies"));
    assert!(should_ignore_line("Yields: 12 bars"));
    assert!(should_ignore_line("Serves 4 generously"));
    assert!(should_ignore_line("Makes 12 muffins"));
    assert!(should_ignore_line("Ingredients"));
    assert!(should_ignore_line("ingredients"));
    assert!(should_ignore_line("  ingredients  "));
    assert!(should_ignore_line("Serve with"));
    assert!(should_ignore_line("serve with"));
    // All-caps versions are intentionally NOT filtered so that
    // detect_section_header can promote them to section transitions.
    assert!(!should_ignore_line("INGREDIENTS"));
    assert!(!should_ignore_line("SERVE WITH"));

    // Regular ingredients should not be ignored
    assert!(!should_ignore_line("1 cup flour"));
    assert!(!should_ignore_line("salt to taste"));
    assert!(!should_ignore_line("For the sauce:"));
    assert!(!should_ignore_line(
        "40 grams raspberries (about 10 raspberries)"
    ));
    assert!(!should_ignore_line("(about 10 raspberries)"));
    // Longer phrases containing the same words must still pass through.
    assert!(!should_ignore_line("Cake Ingredients:"));
    assert!(!should_ignore_line("Serve with crackers"));
}

#[test]
fn test_should_ignore_equipment_lines() {
    // Equipment-only lines should be ignored
    assert!(should_ignore_line(
        "A 12-cup Bundt pan, a pastry bag, and a large star tip"
    ));
    assert!(should_ignore_line(
        "A 9½\"-diameter tart pan with removable bottom"
    ));
    assert!(should_ignore_line(
        "Large dutch oven, pot, or high-sided skillet."
    ));
    assert!(should_ignore_line(
        "A dutch oven or large heavy-bottomed pot"
    ));
    assert!(should_ignore_line(
        "9  \" wide cake pan with 2\" tall sides."
    ));
    assert!(should_ignore_line(
        "2 9x13 metal baking pans or (1) large roasting pan lined with foil."
    ));
    assert!(should_ignore_line("11.38 x 16.5 lipped baking pan"));
    assert!(should_ignore_line("6 QT Slow Cooker"));
    assert!(should_ignore_line(
        "Hand immersion blender (preferred & easier, or food processor, or blender)"
    ));
    assert!(should_ignore_line("stand mixer"));
    assert!(should_ignore_line("food processor"));
    assert!(should_ignore_line(
        "1 large disposable aluminum roasting pan (if using charcoal)"
    ));

    // Ingredient lines mentioning equipment should NOT be ignored
    assert!(!should_ignore_line("Pan drippings from roasting pan"));
    assert!(!should_ignore_line("oil, for the pan"));
    assert!(!should_ignore_line("butter for the pan"));
    assert!(!should_ignore_line("pan spray"));
    assert!(!should_ignore_line("Parchment paper"));
    assert!(!should_ignore_line(
        "4 18×13-inch (approximately) pieces parchment paper"
    ));
    assert!(!should_ignore_line(
        "8  16-inch long pieces of Reynolds Wrap Heavy Duty Foil"
    ));
    assert!(!should_ignore_line("Vegetable oil, for greasing pan"));
    assert!(!should_ignore_line("2 cups pan drippings liquid"));
    assert!(!should_ignore_line(
        "ghee (butter or coconut oil (to coat skillet))"
    ));
    assert!(!should_ignore_line(
        "2 cups panko breadcrumbs, pulsed in a food processor"
    ));
    assert!(!should_ignore_line(
        "1 tsp caraway seeds, (toasted and ground (use food processor))"
    ));
}

#[test]
fn test_title_case() {
    assert_eq!(title_case("FILLING"), "Filling");
    assert_eq!(title_case("FOR THE SAUCE"), "For the Sauce");
    assert_eq!(title_case("TOPPINGS, OPTIONAL"), "Toppings, Optional");
    assert_eq!(title_case("for the dough"), "For the Dough");
    assert_eq!(
        title_case("FOR THE STEAK FAJITA MARINADE"),
        "For the Steak Fajita Marinade"
    );
}

#[test]
fn test_is_only_prep_words() {
    // Single prep words
    assert!(is_only_prep_words("sliced"));
    assert!(is_only_prep_words("chopped"));
    assert!(is_only_prep_words("cooked"));
    assert!(is_only_prep_words("toasted"));
    assert!(is_only_prep_words("roasted"));

    // Multiple prep words
    assert!(is_only_prep_words("finely chopped"));
    assert!(is_only_prep_words("thinly sliced"));

    // Not only prep words (contains actual ingredient)
    assert!(!is_only_prep_words("cooked chicken"));
    assert!(!is_only_prep_words("chicken"));
    assert!(!is_only_prep_words("sliced onions"));

    // Edge cases
    assert!(is_only_prep_words("")); // empty returns true (no non-prep words)
    assert!(!is_only_prep_words("salt"));
    assert!(!is_only_prep_words("butter"));
}

#[test]
fn test_trailing_prep_note_requires_prep_phrase_not_noun_phrase() {
    assert!(is_trailing_prep_note("finely chopped"));
    assert!(is_trailing_prep_note("peeled and chopped"));
    assert!(is_trailing_prep_note("chopped very small"));
    assert!(is_trailing_prep_note("scrubbed clean"));
    assert!(is_trailing_prep_note("scrubbed clean but not peeled"));
    assert!(is_trailing_prep_note("thinly sliced for garnish"));
    assert!(is_trailing_prep_note("cooled to room temperature"));
    assert!(!is_trailing_prep_note("unseasoned dried breadcrumbs"));
    assert!(!is_trailing_prep_note("cooked chicken meat"));
}

#[test]
fn test_trailing_guidance_note_allows_non_prep_suffixes() {
    assert!(is_trailing_guidance_note("or to taste"));
    assert!(is_trailing_guidance_note("or more to taste"));
    assert!(is_trailing_guidance_note("more for serving"));
    assert!(is_trailing_guidance_note("plus extra for seasoning"));
    assert!(!is_trailing_guidance_note("unseasoned dried breadcrumbs"));
}

#[test]
fn test_orphaned_prep_word_fallback() {
    // When the item would be just a prep word, return raw
    let result = parse_ingredient("sliced");
    assert_eq!(result.item, "sliced");

    // When comma is between prep words and ingredient, don't split there
    // "2 cups finely chopped, cooked chicken meat" should NOT extract
    // "cooked chicken meat" as note, leaving "finely chopped" as item
    let result = parse_ingredient("2 cups finely chopped, cooked chicken meat");
    // Item should include the ingredient, not just prep words
    assert_eq!(result.item, "finely chopped, cooked chicken meat");
    assert_eq!(result.measurements.len(), 1);
    assert_eq!(result.measurements[0].amount, Some("2".to_string()));
    assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
}

#[test]
fn test_trailing_scrubbed_clean_is_note_not_item_identity() {
    let result = parse_ingredient("4 russet potatoes, scrubbed clean");
    assert_eq!(result.item, "russet potatoes");
    assert_eq!(result.note, Some("scrubbed clean".to_string()));
    assert_eq!(result.measurements.len(), 1);
    assert_eq!(result.measurements[0].amount, Some("4".to_string()));
    assert_eq!(result.measurements[0].unit, None);
}

#[test]
fn test_trailing_guidance_suffix_is_note_not_item_identity() {
    let result = parse_ingredient("0.25 cup pasta sauce, or to taste");
    assert_eq!(result.item, "pasta sauce");
    assert_eq!(result.note, Some("or to taste".to_string()));
    assert_eq!(result.measurements.len(), 1);
    assert_eq!(result.measurements[0].amount, Some("0.25".to_string()));
    assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
}

#[test]
fn test_mixed_trailing_prep_guidance_suffix_is_note_not_item_identity() {
    let result = parse_ingredient("Oranges, thinly sliced for garnish");
    assert_eq!(result.item, "Oranges");
    assert_eq!(result.note, Some("thinly sliced for garnish".to_string()));
    assert!(result.measurements.is_empty());
}

#[test]
fn test_split_compound_items() {
    assert_eq!(
        split_compound_items("salt and pepper"),
        vec!["salt", "pepper"]
    );
    assert_eq!(
        split_compound_items("dried oregano and cumin"),
        vec!["dried oregano", "cumin"]
    );
    assert_eq!(
        split_compound_items("ground cinnamon, ginger, cloves, and cardamom"),
        vec!["ground cinnamon", "ginger", "cloves", "cardamom"]
    );
    assert_eq!(
        split_compound_items("chili powder, onion powder, and garlic powder"),
        vec!["chili powder", "onion powder", "garlic powder"]
    );
    assert_eq!(
        split_compound_items("garlic and onion powder"),
        vec!["garlic", "onion powder"]
    );
    // Single item - no split
    assert_eq!(
        split_compound_items("pork tenderloins"),
        vec!["pork tenderloins"]
    );
}

#[test]
fn test_should_ignore_standalone_asterisks() {
    assert!(should_ignore_line("**"));
    assert!(should_ignore_line("*"));
    assert!(should_ignore_line("***"));
    // Regular ingredients should not be ignored
    assert!(!should_ignore_line("1 cup flour*"));
}
