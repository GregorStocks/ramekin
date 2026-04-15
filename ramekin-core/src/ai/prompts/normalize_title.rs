//! Normalize-title prompt: de-clickbait a recipe name into a plain description.

/// Prompt name for cache keys.
pub const NORMALIZE_TITLE_PROMPT_NAME: &str = "normalize_title";

/// Render the normalize-title prompt for the given recipe.
///
/// Passes the full ingredient list and instructions (truncated to something
/// reasonable) so the model can disambiguate which dish the title refers to
/// and decide whether a clarification is warranted.
pub fn render_normalize_title_prompt(title: &str, ingredients: &str, instructions: &str) -> String {
    // Truncate instructions: we only need enough context for disambiguation.
    let instructions_trimmed: String = instructions.chars().take(1500).collect();
    format!(
        r#"You are a recipe title editor. Given a recipe title, rewrite it as a plain, descriptive name of the dish.

What to REMOVE:
- Marketing words and superlatives ("Best Ever", "Ultimate", "Amazing", "Perfect", "Crazy Good")
- Personal framing ("My Favorite", "Grandma's", "Mom's Secret")
- Audience qualifiers ("For Adults", "For Kids", "For a Crowd", "(You'll Love These!)")
- Time/effort promises ("15-Minute", "Easy", "Quick", "Weeknight", "One-Pot" when purely promotional — keep it if it's a defining method, e.g. "Instant Pot")
- Emojis and story padding

What to KEEP (do NOT strip these):
- Parenthetical clarifications that explain what the dish is. "Baked Rajma (Punjabi-Style Red Beans with Cream)" stays exactly as is.
- Full ingredient lists connected by "and" or commas. Do not collapse them or drop punctuation. "Asparagus, Goat Cheese and Lemon Pasta" stays as is — do NOT rewrite it to "Asparagus Goat Cheese Lemon Pasta".
- Hyphenation and punctuation from the original when it's grammatically standard. "Bangers and Sweet-Potato Mash with Caramelized Onions" stays as is — do NOT drop the hyphen.
- Cuisine, cooking method, and notable variation (e.g. "Instant Pot", "Sichuan", "Vegan").

What to ADD:
- For genuinely obscure dish names that most English-speaking home cooks wouldn't recognize (e.g. "Billi Bi", "Colcannon", "Ful Medames", "Khao Soi"), add a short parenthetical clarification. Use the format "Authentic Name (Clarification)". Example: "Billi Bi" -> "Billi Bi (Cream of Mussel Soup)", "Colcannon" -> "Colcannon (Irish Mashed Potatoes with Cabbage)".
- Do NOT add clarifications for dishes an average home cook would already know (pad thai, risotto, ratatouille, hummus, bibimbap, etc.).

What to NORMALIZE:
- If the original already has a clarification but in a different shape — e.g. "Cream of Mussel Soup (Billi Bi)", "Billi Bi - Cream of Mussel Soup", "Colcannon, Irish Mashed Potatoes" — rewrite it into the canonical "Authentic Name (Clarification)" form: "Billi Bi (Cream of Mussel Soup)".
- If the original uses a translated/descriptive name as the headline with the authentic name parenthesized or appended, swap them so the authentic name comes first and the clarification goes in parentheses.
- Use the recipe ingredients and instructions to confirm which dish is actually being described before deciding on the authentic name.

General rules:
- When in doubt, change LESS. Preserve the original wording, punctuation, and hyphenation.
- Do not invent ingredients or change which dish it is.
- Use title case only if the original already uses it; otherwise preserve the original casing style.

Examples:
- "My Favorite Bean Burger For Adults" -> "Bean Burger"
- "The Best Ever Chewy Chocolate Chip Cookies (You'll Love These!)" -> "Chewy Chocolate Chip Cookies"
- "Grandma's Secret Weeknight Instant Pot Butter Chicken" -> "Instant Pot Butter Chicken"
- "EASY 15-Minute Garlic Shrimp Pasta" -> "Garlic Shrimp Pasta"
- "Baked Rajma (Punjabi-Style Red Beans with Cream)" -> "Baked Rajma (Punjabi-Style Red Beans with Cream)"
- "Asparagus, Goat Cheese and Lemon Pasta" -> "Asparagus, Goat Cheese and Lemon Pasta"
- "Bangers and Sweet-Potato Mash with Caramelized Onions" -> "Bangers and Sweet-Potato Mash with Caramelized Onions"
- "Billi Bi" -> "Billi Bi (Cream of Mussel Soup)"
- "Colcannon" -> "Colcannon (Irish Mashed Potatoes with Cabbage)"

Recipe:
- Title: {title}
- Ingredients: {ingredients}
- Instructions: {instructions}

Respond with JSON only, no other text: {{"normalized_title": "..."}}"#,
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
        let prompt = render_normalize_title_prompt(
            "My Favorite Bean Burger For Adults",
            "black beans, breadcrumbs, egg",
            "Mash beans, mix, form patties, fry.",
        );
        assert!(prompt.contains("My Favorite Bean Burger For Adults"));
        assert!(prompt.contains("black beans"));
        assert!(prompt.contains("normalized_title"));
    }
}
