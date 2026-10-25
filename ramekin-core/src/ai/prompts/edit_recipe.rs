//! Edit with AI prompt for applying user-described changes to recipes.

/// Prompt name for cache keys.
pub const EDIT_RECIPE_PROMPT_NAME: &str = "edit_recipe";

/// Render the system prompt for an AI recipe edit.
pub fn render_edit_recipe_system_prompt() -> String {
    r#"You edit a recipe the way the user asks, using tools. You see the recipe with its ingredients and steps numbered. The user may also attach photos of the dish as context.

How to work:
- Change only what the request needs. Every edit tool changes one thing; anything you don't touch stays exactly as it is.
- Look things up rather than guessing: lookup_ingredient gives catalog grams per cup, grams per piece and calories per 100 g, for unit conversions and calorie targets. scale_recipe multiplies every amount at once. list_tags and search_recipes show the user's own tags and recipes.
- Numbers shift after you add or remove an ingredient or step; the tool's reply says so. Keep track, or work from the end of the list backwards.
- Write ingredient lines the way a recipe would ("2 cups flour, sifted"); they are parsed into amount, unit, item and note.
- When the recipe says what the user asked for, call finish with a short summary of what you changed. If the request can't be done, change nothing and say why in the summary."#
        .to_string()
}

/// Render the user message with the recipe and the request.
pub fn render_edit_recipe_user_prompt(recipe: &str, instruction: &str) -> String {
    format!(
        "Here is the recipe:\n\n{recipe}\nIf photos are attached, use them as context when they help.\n\nThe change I want: {instruction}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_system_prompt() {
        let prompt = render_edit_recipe_system_prompt();
        assert!(prompt.contains("You edit a recipe"));
        assert!(prompt.contains("finish"));
    }

    #[test]
    fn test_render_user_prompt() {
        let prompt = render_edit_recipe_user_prompt("Title: Chicken Stir Fry\n", "make this vegan");
        assert!(prompt.contains("Chicken Stir Fry"));
        assert!(prompt.contains("make this vegan"));
    }
}
