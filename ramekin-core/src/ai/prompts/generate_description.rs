//! Generate-description prompt: produce a concise menu-style description for a recipe.

/// Prompt name for cache keys.
pub const GENERATE_DESCRIPTION_PROMPT_NAME: &str = "generate_description";

/// Render the generate-description prompt for the given recipe.
///
/// Passes the title, ingredient list, and instructions so the model can
/// write an accurate, appetizing menu-blurb description that fits on a
/// printed recipe card (~70 character budget).
pub fn render_generate_description_prompt(
    title: &str,
    ingredients: &str,
    instructions: &str,
) -> String {
    // Truncate instructions: we only need enough context to understand the dish.
    let instructions_trimmed: String = instructions.chars().take(1500).collect();
    format!(
        r#"You are writing a short description for a recipe, like what you'd see on a restaurant menu.

Rules:
- HARD LIMIT: 70 characters total. Aim for 45-65. Count carefully.
- A single sentence fragment, like a tight menu blurb
- Mention one or two key flavors, textures, or techniques — not all of them
- Do not repeat the recipe title verbatim
- Do not use marketing language ("amazing", "the best", "to die for", "festive", "vibrant", "hearty")
- Do not mention cook time, difficulty, or serving size
- Skip filler openings like "A …", "Featuring …", "Made with …" — start with the descriptive substance
- Examples of the right length and style (each under 70 chars):
  - "Tender short ribs braised in red wine over creamy polenta."
  - "Crisp-skinned salmon with ginger-dill butter."
  - "Cabbage soup with rye croutons and smoked paprika."
  - "Vanilla bean custard with a crisp caramelized sugar crust."
- If the dish is well-known (e.g. "Baked Potatoes"), 3-5 words is fine

Recipe:
- Name: {title}
- Ingredients: {ingredients}
- Steps: {instructions}

Respond with JSON only, no other text: {{"description": "..."}}"#,
        title = title,
        ingredients = ingredients,
        instructions = instructions_trimmed,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_prompt() {
        let prompt = render_generate_description_prompt(
            "Garlic Shrimp Pasta",
            "shrimp, garlic, linguine, butter, white wine, parsley",
            "Cook pasta. Sauté garlic in butter, add shrimp, deglaze with wine.",
        );
        assert!(prompt.contains("Garlic Shrimp Pasta"));
        assert!(prompt.contains("shrimp"));
        assert!(prompt.contains("description"));
    }
}
