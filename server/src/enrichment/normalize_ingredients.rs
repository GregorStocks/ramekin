//! NormalizeIngredients enrichment.
//!
//! Standardizes ingredient amounts, units, and names into a consistent format.

use super::{Enrichment, EnrichmentError};
use crate::models::Ingredient;
use crate::types::RecipeContent;
use async_trait::async_trait;

/// Enrichment that normalizes ingredient lists.
///
/// Takes raw ingredient text and converts it to structured data with
/// standardized amounts, units, and item names.
#[derive(Debug, Clone, Copy)]
pub struct NormalizeIngredients;

#[async_trait]
impl Enrichment for NormalizeIngredients {
    fn enrichment_type(&self) -> &'static str {
        "normalize_ingredients"
    }

    fn display_name(&self) -> &'static str {
        "Normalize Ingredients"
    }

    fn description(&self) -> &'static str {
        "Standardize ingredient amounts, units, and names"
    }

    fn output_fields(&self) -> &'static [&'static str] {
        &["ingredients"]
    }

    fn build_prompt(&self, recipe: &RecipeContent) -> String {
        let ingredients_text = recipe
            .ingredients
            .iter()
            .map(format_ingredient_for_prompt)
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"Parse and normalize these recipe ingredients into structured JSON.

For each ingredient, extract:
- amount: The numeric quantity (e.g., "1", "1/2", "2-3"). Use null if not specified.
- unit: The unit of measurement, standardized (e.g., "cup", "tablespoon", "teaspoon", "pound", "ounce", "gram"). Use null if not specified (e.g., "2 eggs").
- item: The ingredient name, cleaned up and standardized.
- note: Any preparation notes or modifiers (e.g., "diced", "softened", "optional"). Use null if none.

Standardization rules:
- Convert "tbsp" to "tablespoon", "tsp" to "teaspoon"
- Convert "lb" to "pound", "oz" to "ounce"
- Keep amounts as strings to preserve fractions like "1/2"
- Move preparation instructions from the item to the note field

Ingredients:
{ingredients_text}

Respond with ONLY a JSON array of objects, no other text. Example format:
[
  {{"amount": "1", "unit": "cup", "item": "all-purpose flour", "note": null}},
  {{"amount": "2", "unit": "tablespoon", "item": "butter", "note": "softened"}}
]"#
        )
    }

    fn apply_response(
        &self,
        original: &RecipeContent,
        response: &str,
    ) -> Result<RecipeContent, EnrichmentError> {
        let mut result = original.clone();

        // Parse the JSON response
        let parsed_ingredients: Vec<IngredientJson> = serde_json::from_str(response.trim())
            .map_err(|e| {
                EnrichmentError::Parse(format!("Invalid JSON: {} - Response was: {}", e, response))
            })?;

        // Convert to our Ingredient type
        let ingredients: Vec<Ingredient> = parsed_ingredients
            .into_iter()
            .map(|ing| Ingredient {
                amount: ing.amount,
                unit: ing.unit,
                item: ing.item,
                note: ing.note,
            })
            .collect();

        // Validate we got at least some ingredients
        if ingredients.is_empty() && !original.ingredients.is_empty() {
            return Err(EnrichmentError::Validation(
                "Enrichment produced empty ingredients list".to_string(),
            ));
        }

        result.ingredients = ingredients;
        Ok(result)
    }
}

/// Helper struct for parsing JSON response.
#[derive(Debug, serde::Deserialize)]
struct IngredientJson {
    amount: Option<String>,
    unit: Option<String>,
    item: String,
    note: Option<String>,
}

/// Format an ingredient for the prompt.
fn format_ingredient_for_prompt(ing: &Ingredient) -> String {
    let mut parts = Vec::new();

    if let Some(ref amount) = ing.amount {
        parts.push(amount.clone());
    }
    if let Some(ref unit) = ing.unit {
        parts.push(unit.clone());
    }
    parts.push(ing.item.clone());
    if let Some(ref note) = ing.note {
        parts.push(format!("({})", note));
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_recipe() -> RecipeContent {
        RecipeContent {
            title: "Test Recipe".to_string(),
            description: None,
            ingredients: vec![
                Ingredient {
                    amount: Some("1".to_string()),
                    unit: Some("cup".to_string()),
                    item: "flour".to_string(),
                    note: None,
                },
                Ingredient {
                    amount: Some("2".to_string()),
                    unit: Some("tbsp".to_string()),
                    item: "butter, softened".to_string(),
                    note: None,
                },
            ],
            instructions: "Mix ingredients.".to_string(),
            source_url: None,
            source_name: None,
            tags: vec![],
            servings: None,
            prep_time: None,
            cook_time: None,
            total_time: None,
            rating: None,
            difficulty: None,
            nutritional_info: None,
            notes: None,
        }
    }

    #[test]
    fn test_build_prompt() {
        let enrichment = NormalizeIngredients;
        let recipe = sample_recipe();
        let prompt = enrichment.build_prompt(&recipe);

        assert!(prompt.contains("1 cup flour"));
        assert!(prompt.contains("2 tbsp butter, softened"));
        assert!(prompt.contains("JSON array"));
    }

    #[test]
    fn test_apply_response_valid() {
        let enrichment = NormalizeIngredients;
        let recipe = sample_recipe();

        let response = r#"[
            {"amount": "1", "unit": "cup", "item": "all-purpose flour", "note": null},
            {"amount": "2", "unit": "tablespoon", "item": "butter", "note": "softened"}
        ]"#;

        let result = enrichment.apply_response(&recipe, response).unwrap();

        assert_eq!(result.ingredients.len(), 2);
        assert_eq!(result.ingredients[0].item, "all-purpose flour");
        assert_eq!(result.ingredients[1].unit, Some("tablespoon".to_string()));
        assert_eq!(result.ingredients[1].note, Some("softened".to_string()));
    }

    #[test]
    fn test_apply_response_invalid_json() {
        let enrichment = NormalizeIngredients;
        let recipe = sample_recipe();

        let response = "not valid json";
        let result = enrichment.apply_response(&recipe, response);

        assert!(result.is_err());
    }

    #[test]
    fn test_output_fields() {
        let enrichment = NormalizeIngredients;
        assert_eq!(enrichment.output_fields(), &["ingredients"]);
    }
}
