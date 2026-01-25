//! AddNutrition enrichment.
//!
//! Adds nutritional information to recipes based on ingredients.

use super::{Enrichment, EnrichmentError};
use crate::llm::LlmProvider;
use crate::types::RecipeContent;
use async_trait::async_trait;

/// Enrichment that adds nutritional information.
///
/// Analyzes recipe ingredients and estimates calories, protein,
/// carbohydrates, fat, and other nutritional data.
#[derive(Debug, Clone, Copy)]
pub struct AddNutrition;

#[async_trait]
impl Enrichment for AddNutrition {
    fn enrichment_type(&self) -> &'static str {
        "add_nutrition"
    }

    fn display_name(&self) -> &'static str {
        "Add Nutrition Info"
    }

    fn description(&self) -> &'static str {
        "Estimate nutritional information based on ingredients"
    }

    fn output_fields(&self) -> &'static [&'static str] {
        &["nutritional_info"]
    }

    fn build_prompt(&self, _recipe: &RecipeContent) -> String {
        // Not implemented yet
        String::new()
    }

    fn apply_response(
        &self,
        _original: &RecipeContent,
        _response: &str,
    ) -> Result<RecipeContent, EnrichmentError> {
        Err(EnrichmentError::Validation(
            "AddNutrition enrichment not yet implemented".to_string(),
        ))
    }

    async fn run(
        &self,
        _provider: &dyn LlmProvider,
        _recipe: &RecipeContent,
    ) -> Result<RecipeContent, EnrichmentError> {
        Err(EnrichmentError::Validation(
            "AddNutrition enrichment not yet implemented".to_string(),
        ))
    }
}
