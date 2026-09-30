//! Resolve-ingredient-names prompt: map names the catalog doesn't know to a
//! catalog key from a shortlist, or say it isn't food / can't tell.

/// Prompt name for cache keys.
pub const RESOLVE_INGREDIENT_NAMES_PROMPT_NAME: &str = "resolve_ingredient_names";

/// Render the prompt for a batch of names, each with its candidate keys.
pub fn render_resolve_ingredient_names_prompt(names: &[(String, Vec<String>)]) -> String {
    let items = serde_json::to_string_pretty(
        &names
            .iter()
            .map(|(name, candidates)| serde_json::json!({ "name": name, "candidates": candidates }))
            .collect::<Vec<_>>(),
    )
    .expect("names serialize");
    format!(
        r#"You are an ingredient name resolver for a recipe app's food catalog. Each item below is an ingredient name from a recipe that the catalog doesn't recognize, with a shortlist of catalog keys that share words with it.

For each item, answer exactly one of:
- "entry": the name means the same food (or purchasable product) as one of ITS candidates. Give that candidate as "key", copied exactly. Pick the same food in the form the recipe means (fresh vs dried, raw vs cooked, the specific part such as yolk or zest). Brand or style words don't change the food.
- "not_food": the name isn't an ingredient at all (a heading, an instruction, a serving note, a yield).
- "unknown": none of its candidates is the same food, or you aren't sure. Prefer "unknown" over a guess; a wrong food corrupts calorie estimates.

Never answer with a key that isn't in that item's candidates, and never invent one.

Items:
{items}

Respond with JSON: {{"resolutions": [{{"name": "<name exactly as given>", "answer": "entry" | "not_food" | "unknown", "key": "<candidate, only for entry>"}}]}}, with one resolution for every item."#
    )
}
