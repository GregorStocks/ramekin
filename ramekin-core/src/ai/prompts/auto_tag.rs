//! Auto-tag prompt for suggesting recipe tags based on user's existing tags.

/// Prompt name for cache keys.
pub const AUTO_TAG_PROMPT_NAME: &str = "auto_tag";

/// Render the auto-tag prompt with the given recipe data and existing tags.
pub fn render_auto_tag_prompt(
    title: &str,
    ingredients: &str,
    instructions: &str,
    existing_tags: &[String],
) -> String {
    let tags_list = existing_tags.join(", ");

    format!(
        r#"You are a recipe tagging assistant. Given a recipe and the user's existing tags, suggest which tags from their list would apply to this recipe.

IMPORTANT: Only suggest tags from the provided list. Never create new tags.

Recipe:
- Title: {title}
- Ingredients: {ingredients}
- Instructions: {instructions}

User's existing tags: {tags_list}

Respond with JSON only, no other text: {{"suggested_tags": ["tag1", "tag2"]}}"#,
        title = title,
        ingredients = ingredients,
        instructions = instructions,
        tags_list = tags_list
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_prompt() {
        let prompt = render_auto_tag_prompt(
            "Chicken Stir Fry",
            "chicken, soy sauce, vegetables",
            "Cook the chicken, add vegetables...",
            &[
                "dinner".to_string(),
                "quick".to_string(),
                "asian".to_string(),
            ],
        );

        assert!(prompt.contains("Chicken Stir Fry"));
        assert!(prompt.contains("dinner, quick, asian"));
        assert!(prompt.contains("suggested_tags"));
    }
}
