//! Resolve-ingredient-names prompt: map names the catalog doesn't know (or
//! calls ambiguous) to a catalog key from a shortlist, estimate a real food no
//! candidate matches, or say it isn't food / can't tell.

/// Prompt name for cache keys.
pub const RESOLVE_INGREDIENT_NAMES_PROMPT_NAME: &str = "resolve_ingredient_names";

/// One name to resolve: its candidate catalog keys, and whether the catalog
/// calls it ambiguous (a generic name such as "cheese").
pub struct NameQuery {
    pub name: String,
    pub candidates: Vec<String>,
    pub ambiguous: bool,
}

/// Render the prompt for a batch of names.
pub fn render_resolve_ingredient_names_prompt(names: &[NameQuery]) -> String {
    let items = serde_json::to_string_pretty(
        &names
            .iter()
            .map(|query| {
                serde_json::json!({
                    "name": query.name,
                    "candidates": query.candidates,
                    "ambiguous": query.ambiguous,
                })
            })
            .collect::<Vec<_>>(),
    )
    .expect("names serialize");
    format!(
        r#"You are an ingredient name resolver for a recipe app's food catalog. Each item below is an ingredient name from a recipe, with a shortlist of catalog keys that share words with it. Most items are names the catalog doesn't recognize; items marked "ambiguous": true are generic names the catalog recognizes but can't pin to one food (such as "cheese" or "oil").

For each item, answer exactly one of:
- "entry": the name means the same food (or purchasable product) as one of ITS candidates. Give that candidate as "key", copied exactly. Pick the same food in the form the recipe means (fresh vs dried, raw vs cooked, the specific part such as yolk or zest). Brand or style words don't change the food. For an ambiguous item, pick the food a typical US home recipe most likely means by that generic name.
- "estimate": only for an item NOT marked ambiguous that is a real food or drink none of its candidates matches (a regional ingredient, a branded product, a prepared dish). Give your best typical values: "kcal_per_100g" (number), "grams_per_cup" (grams in one level US cup as a recipe would measure it, or null if it isn't measured by volume), and "grams_per_piece" (grams in one whole typical item, where a single cut or portion such as a chop, steak, fillet, leg, thigh or slice counts as one item; null only if recipes never count it).
- "not_food": only for an item NOT marked ambiguous, when the name isn't an ingredient to count: a section heading ("toppings", "for the sauce", "assembly"), an instruction or serving note ("to serve", "for garnish"), a yield, equipment ("lid", "parchment"), a leftover fragment of a line with no food in it (a bare preparation or size word such as "packed", "finely grated", "tiny"), or a reference to something prepared earlier in the same recipe ("sauce from above", "the reserved marinade"), which counting would double count.
- "unknown": you can't tell what food it is. Prefer "unknown" over a guess when the name itself is unclear; a wrong food corrupts calorie estimates.

Never answer "entry" with a key that isn't in that item's candidates, and never invent one.

Items:
{items}

Respond with JSON: {{"resolutions": [{{"name": "<name exactly as given>", "answer": "entry" | "estimate" | "not_food" | "unknown", "key": "<candidate, only for entry>", "kcal_per_100g": <number, only for estimate>, "grams_per_cup": <number or null, only for estimate>, "grams_per_piece": <number or null, only for estimate>}}]}}, with one resolution for every item."#
    )
}
