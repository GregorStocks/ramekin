//! Prompt template for recipe photo generation.

/// Render the prompt used to generate a recipe hero image.
pub fn render_generate_recipe_photo_prompt(
    title: &str,
    description: Option<&str>,
    ingredients: &str,
    instructions: &str,
) -> String {
    let description = description.unwrap_or("None");

    format!(
        r#"Create a single appetizing, realistic hero photo of the finished dish for this recipe.

Recipe:
- Title: {title}
- Description: {description}
- Ingredients: {ingredients}
- Instructions: {instructions}

Requirements:
- Show the plated finished dish only, not raw ingredients or preparation steps.
- Use the title, description, ingredients, and instructions to infer the dish accurately.
- Keep the composition suitable for a cookbook or recipe app.
- Frame the image in a landscape 3:2 aspect ratio composition.
- No text, watermarks, cutlery overlays, recipe cards, or split-screen collages.
- Prefer natural lighting and believable food styling over exaggerated ad imagery."#,
        title = title,
        description = description,
        ingredients = ingredients,
        instructions = instructions
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_generate_recipe_photo_prompt() {
        let prompt = render_generate_recipe_photo_prompt(
            "Lemon Pasta",
            Some("A bright, creamy weeknight pasta."),
            "spaghetti, lemon, parmesan, butter",
            "Boil the pasta, toss it with butter, lemon, and parmesan, then serve immediately.",
        );

        assert!(prompt.contains("Lemon Pasta"));
        assert!(prompt.contains("A bright, creamy weeknight pasta."));
        assert!(prompt.contains("spaghetti, lemon, parmesan, butter"));
        assert!(prompt.contains("Boil the pasta, toss it with butter"));
        assert!(prompt.contains("hero photo"));
        assert!(prompt.contains("3:2 aspect ratio"));
    }
}
