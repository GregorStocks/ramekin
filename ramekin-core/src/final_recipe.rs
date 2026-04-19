//! Consolidated end-of-pipeline recipe view.
//!
//! Assembles the final state of a scraped recipe by combining the outputs of
//! `extract_recipe`, `parse_ingredients`, `enrich_auto_tag`, and `apply_auto_tags`.
//! Used by the CLI snapshot writer and (eventually) the server scrape-status view.

use serde::{Deserialize, Serialize};

use crate::ingredient_parser::ParsedIngredient;
use crate::types::RawRecipe;

/// Consolidated end-of-pipeline recipe state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FinalRecipe {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servings: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prep_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cook_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_time: Option<String>,
    pub instructions: String,
    pub image_urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    pub ingredients: Vec<ParsedIngredient>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_tags: Option<Vec<String>>,
}

/// Build a `FinalRecipe` from the outputs of the relevant pipeline steps.
pub fn build_final_recipe(
    raw_recipe: &RawRecipe,
    parsed_ingredients: Option<&[ParsedIngredient]>,
    suggested_tags: Option<&[String]>,
    applied_tags: Option<&[String]>,
) -> FinalRecipe {
    let _ = (parsed_ingredients, suggested_tags, applied_tags);
    FinalRecipe {
        title: raw_recipe.title.clone(),
        description: raw_recipe.description.clone(),
        servings: raw_recipe.servings.clone(),
        prep_time: raw_recipe.prep_time.clone(),
        cook_time: raw_recipe.cook_time.clone(),
        total_time: raw_recipe.total_time.clone(),
        instructions: raw_recipe.instructions.clone(),
        image_urls: raw_recipe.image_urls.clone(),
        source_url: raw_recipe.source_url.clone(),
        source_name: raw_recipe.source_name.clone(),
        ingredients: Vec::new(),
        applied_tags: None,
        suggested_tags: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_recipe_fixture() -> RawRecipe {
        RawRecipe {
            title: "Test Recipe".to_string(),
            description: Some("A test".to_string()),
            ingredients: "1 cup flour\n2 eggs".to_string(),
            instructions: "Mix and bake.".to_string(),
            image_urls: vec!["https://example.com/img.jpg".to_string()],
            source_url: Some("https://example.com/recipe".to_string()),
            source_name: Some("example.com".to_string()),
            servings: Some("4".to_string()),
            prep_time: Some("10 minutes".to_string()),
            cook_time: Some("20 minutes".to_string()),
            total_time: Some("30 minutes".to_string()),
            rating: None,
            difficulty: None,
            nutritional_info: None,
            notes: None,
            categories: None,
            footnotes: None,
        }
    }

    #[test]
    fn copies_raw_recipe_fields() {
        let raw = raw_recipe_fixture();
        let fr = build_final_recipe(&raw, None, None, None);
        assert_eq!(fr.title, "Test Recipe");
        assert_eq!(fr.description.as_deref(), Some("A test"));
        assert_eq!(fr.instructions, "Mix and bake.");
        assert_eq!(
            fr.image_urls,
            vec!["https://example.com/img.jpg".to_string()]
        );
        assert_eq!(fr.source_name.as_deref(), Some("example.com"));
        assert_eq!(fr.servings.as_deref(), Some("4"));
        assert_eq!(fr.total_time.as_deref(), Some("30 minutes"));
    }
}
