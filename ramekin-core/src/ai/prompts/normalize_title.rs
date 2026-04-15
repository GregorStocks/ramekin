//! Normalize-title prompt: de-clickbait a recipe name into a plain description.

/// Prompt name for cache keys.
pub const NORMALIZE_TITLE_PROMPT_NAME: &str = "normalize_title";

/// Render the normalize-title prompt for the given recipe title.
pub fn render_normalize_title_prompt(title: &str) -> String {
    format!(
        r#"You are a recipe title editor. Given a recipe title, rewrite it as a plain, descriptive name of the dish. Strip marketing words, personal framing ("My Favorite", "Best Ever", "The Ultimate"), audience qualifiers ("For Adults", "For Kids", "For a Crowd"), superlatives, emojis, and story padding. Keep distinguishing descriptors that affect what the dish actually is (key ingredients, cooking method, cuisine, notable variation). Use title case. Do not invent ingredients.

Examples:
- "My Favorite Bean Burger For Adults" -> "Bean Burger"
- "The Best Ever Chewy Chocolate Chip Cookies (You'll Love These!)" -> "Chewy Chocolate Chip Cookies"
- "Grandma's Secret Weeknight Instant Pot Butter Chicken" -> "Instant Pot Butter Chicken"
- "EASY 15-Minute Garlic Shrimp Pasta" -> "Garlic Shrimp Pasta"

Title: {title}

Respond with JSON only, no other text: {{"normalized_title": "..."}}"#,
        title = title
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_prompt() {
        let prompt = render_normalize_title_prompt("My Favorite Bean Burger For Adults");
        assert!(prompt.contains("My Favorite Bean Burger For Adults"));
        assert!(prompt.contains("normalized_title"));
    }
}
