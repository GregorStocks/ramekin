//! Estimate-ingredient-weights prompt: grams per unit for catalog foods the
//! catalog can't weigh in that unit (a missing density, or a counted unit
//! with no piece weight).

/// Prompt name for cache keys.
pub const ESTIMATE_INGREDIENT_WEIGHTS_PROMPT_NAME: &str = "estimate_ingredient_weights";

/// Render the prompt for a batch of (food, unit) pairs.
pub fn render_estimate_ingredient_weights_prompt(items: &[(String, String)]) -> String {
    let items = serde_json::to_string_pretty(
        &items
            .iter()
            .map(|(food, unit)| serde_json::json!({ "food": food, "unit": unit }))
            .collect::<Vec<_>>(),
    )
    .expect("items serialize");
    format!(
        r#"You are an ingredient weight estimator for a recipe app's calorie estimates. Each item below is a food from the app's catalog (often a USDA food description) and a unit a recipe measured it in, which the catalog has no weight for.

For each item, give "grams": your best estimate of the weight in grams of ONE of that unit of that food, as typically sold and used in home recipes in the US.
- "cup" means one level US cup (236.6 ml) of the food as a recipe would measure it (chopped, shredded, or whole as the food description suggests).
- "piece" means one whole item of a typical size (one egg, one onion, one cookie).
- Size words ("large", "medium", "small") mean one item of that size.
- Package units ("can", "jar", "package", "bag", "box", "bottle") mean the most common US retail size of that food.
- Other units ("bunch", "head", "clove", "slice", "sprig", "stalk") mean one such unit of a typical size.

Answer null instead of a number when the unit doesn't make sense for the food, or when sizes vary so much that no typical weight exists. A wrong weight corrupts calorie estimates, but a reasonable typical weight is far more useful than null.

Items:
{items}

Respond with JSON: {{"weights": [{{"food": "<food exactly as given>", "unit": "<unit exactly as given>", "grams": <number or null>}}]}}, with one entry for every item."#
    )
}
