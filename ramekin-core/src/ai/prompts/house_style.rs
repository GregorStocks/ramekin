//! House style prompt: rewrites a recipe's ingredients and steps to the
//! owner's style guide, `doc/house-style.md`.

/// Prompt name for cache keys.
pub const HOUSE_STYLE_PROMPT_NAME: &str = "house_style";

/// The style guide, as checked in.
const HOUSE_STYLE_GUIDE: &str = include_str!("../../../../doc/house-style.md");

/// Render the system prompt for a house-style rewrite.
pub fn render_house_style_system_prompt() -> String {
    format!(
        r#"You rewrite a recipe's ingredient list and steps into the house style below, using tools. You see the recipe with its ingredients and steps numbered.

How to work:
- Follow the style guide exactly, and change only what it asks for. A recipe already in house style needs no edits: call finish right away.
- Every edit tool changes one thing; anything you don't touch stays exactly as it is. Rewrite an ingredient line with set_ingredient, reorder with move_ingredient, group components with set_section, and rewrite, add or remove steps with set_step, add_step and remove_step.
- Never change an amount or a unit, and never add, drop or swap an ingredient. Taking prep out of a line ("1 onion, chopped" to "1 onion") must keep the amount and unit exactly.
- Numbers shift after you add, remove or move an ingredient or step; the tool's reply says so. Keep track, or work from the end of the list backwards.
- Write ingredient lines the way a recipe would ("2 tbsp olive oil"); they are parsed into amount, unit, item and note.
- When done, call finish with one or two sentences on what you changed. finish checks that every ingredient and amount is still there, and says what to fix if not.

<style_guide>
{HOUSE_STYLE_GUIDE}
</style_guide>"#
    )
}

/// Render the user message with the recipe.
pub fn render_house_style_user_prompt(recipe: &str) -> String {
    format!("Here is the recipe:\n\n{recipe}\nRewrite it into the house style.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_embeds_the_style_guide() {
        let prompt = render_house_style_system_prompt();
        assert!(prompt.contains("# House style"));
        assert!(prompt.contains("No prep in the line"));
        assert!(prompt.contains("finish"));
    }

    #[test]
    fn user_prompt_carries_the_recipe() {
        let prompt = render_house_style_user_prompt("Title: Soup\n");
        assert!(prompt.contains("Title: Soup"));
    }
}
