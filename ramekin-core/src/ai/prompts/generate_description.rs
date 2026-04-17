//! Generate-description prompt: produce a concise menu-style description for a recipe.

/// Prompt name for cache keys.
pub const GENERATE_DESCRIPTION_PROMPT_NAME: &str = "generate_description";

/// Render the generate-description prompt for the given recipe.
///
/// Passes the title, ingredient list, and instructions so the model can
/// write an accurate, appetizing one-to-two sentence description.
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
- One or two sentences maximum
- Concise and descriptive — focus on what makes the dish appealing
- Mention key flavors, textures, or techniques when relevant
- Do not repeat the recipe title verbatim
- Do not use marketing language ("amazing", "the best", "to die for")
- Do not mention cook time, difficulty, or serving size
- Write in sentence fragments if natural (like a real menu), e.g. "Tender braised short ribs in a rich red wine sauce, served over creamy polenta."
- If the dish is straightforward and well-known (e.g. "Baked Potatoes"), keep it very short

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
