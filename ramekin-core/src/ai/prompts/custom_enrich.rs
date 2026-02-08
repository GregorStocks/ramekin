//! Custom enrichment prompt for applying user-specified changes to recipes.

/// Prompt name for cache keys.
pub const CUSTOM_ENRICH_PROMPT_NAME: &str = "custom_enrich";

/// Render the system prompt for custom enrichment.
pub fn render_custom_enrich_system_prompt() -> String {
    r#"You are a recipe modification assistant. The user will give you a recipe as JSON and an instruction describing what change they want. Apply the requested change and return the complete modified recipe as JSON.

IMPORTANT RULES:
- Return ONLY valid JSON matching the exact schema below. No other text.
- Return the COMPLETE recipe, not just the changed parts.
- Preserve all fields that aren't affected by the change.
- For ingredients, preserve the structure exactly (measurements array with amount/unit, note, raw, section fields).

JSON Schema:
{
  "title": "string",
  "description": "string or null",
  "ingredients": [
    {
      "item": "string",
      "measurements": [{"amount": "string or null", "unit": "string or null"}],
      "note": "string or null",
      "raw": "string or null",
      "section": "string or null"
    }
  ],
  "instructions": "string",
  "source_url": "string or null",
  "source_name": "string or null",
  "tags": ["string"],
  "servings": "string or null",
  "prep_time": "string or null",
  "cook_time": "string or null",
  "total_time": "string or null",
  "rating": "integer or null",
  "difficulty": "string or null",
  "nutritional_info": "string or null",
  "notes": "string or null"
}"#
    .to_string()
}

/// Render the user message with the recipe and instruction.
pub fn render_custom_enrich_user_prompt(recipe_json: &str, instruction: &str) -> String {
    format!(
        "Here is the recipe:\n\n{recipe_json}\n\nApply this change: {instruction}",
        recipe_json = recipe_json,
        instruction = instruction
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_system_prompt() {
        let prompt = render_custom_enrich_system_prompt();
        assert!(prompt.contains("recipe modification assistant"));
        assert!(prompt.contains("JSON Schema"));
        assert!(prompt.contains("ingredients"));
    }

    #[test]
    fn test_render_user_prompt() {
        let recipe = r#"{"title": "Chicken Stir Fry", "instructions": "Cook chicken"}"#;
        let instruction = "make this vegan";
        let prompt = render_custom_enrich_user_prompt(recipe, instruction);
        assert!(prompt.contains("Chicken Stir Fry"));
        assert!(prompt.contains("make this vegan"));
    }
}
