//! Recipe enrichment system.
//!
//! This module provides a trait-based abstraction for enriching recipes using LLMs.
//! Each enrichment type is a struct that implements the `Enrichment` trait.

mod add_nutrition;
mod improve_instructions;
mod normalize_ingredients;
mod normalize_times;

pub use add_nutrition::AddNutrition;
pub use improve_instructions::ImproveInstructions;
pub use normalize_ingredients::NormalizeIngredients;
pub use normalize_times::NormalizeTimes;

use crate::llm::{LlmError, LlmProvider};
use crate::types::RecipeContent;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error type for enrichment operations.
#[derive(Debug, Error)]
pub enum EnrichmentError {
    #[error("LLM error: {0}")]
    Llm(#[from] LlmError),

    #[error("Failed to parse LLM response: {0}")]
    Parse(String),

    #[error("Enrichment produced invalid data: {0}")]
    Validation(String),
}

/// Information about an enrichment type for the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct EnrichmentInfo {
    #[serde(rename = "type")]
    pub enrichment_type: String,
    pub display_name: String,
    pub description: String,
    pub output_fields: Vec<String>,
}

/// Trait for recipe enrichment types.
///
/// Each enrichment type is a stateless struct that implements this trait.
/// All logic for one enrichment lives in its implementation.
#[async_trait]
pub trait Enrichment: Send + Sync + std::fmt::Debug {
    /// Type identifier for serialization/API (e.g., "normalize_ingredients").
    fn enrichment_type(&self) -> &'static str;

    /// Display name for UI (e.g., "Normalize Ingredients").
    fn display_name(&self) -> &'static str;

    /// Description for UI tooltips.
    fn description(&self) -> &'static str;

    /// Which recipe fields this enrichment is allowed to modify.
    fn output_fields(&self) -> &'static [&'static str];

    /// Build the prompt for the LLM.
    ///
    /// Should only include relevant parts of the recipe, not the entire object.
    fn build_prompt(&self, recipe: &RecipeContent) -> String;

    /// Parse LLM response and apply to recipe.
    ///
    /// Must start from `original.clone()` and only modify fields listed in `output_fields()`.
    fn apply_response(
        &self,
        original: &RecipeContent,
        response: &str,
    ) -> Result<RecipeContent, EnrichmentError>;

    /// Run the full enrichment (build prompt, call LLM, apply response).
    async fn run(
        &self,
        provider: &dyn LlmProvider,
        recipe: &RecipeContent,
    ) -> Result<RecipeContent, EnrichmentError> {
        let prompt = self.build_prompt(recipe);
        let response = provider.complete(&prompt).await?;
        self.apply_response(recipe, &response)
    }

    /// Get enrichment info for the API.
    fn info(&self) -> EnrichmentInfo {
        EnrichmentInfo {
            enrichment_type: self.enrichment_type().to_string(),
            display_name: self.display_name().to_string(),
            description: self.description().to_string(),
            output_fields: self.output_fields().iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// Registry of all available enrichment types.
///
/// Add new enrichment types here when implementing them.
pub static ALL_ENRICHMENTS: &[&dyn Enrichment] = &[
    &NormalizeIngredients,
    &NormalizeTimes,
    &AddNutrition,
    &ImproveInstructions,
];

/// Get an enrichment by type name.
pub fn get_enrichment(type_name: &str) -> Option<&'static dyn Enrichment> {
    ALL_ENRICHMENTS
        .iter()
        .find(|e| e.enrichment_type() == type_name)
        .copied()
}

/// Get info for all available enrichments.
pub fn all_enrichment_info() -> Vec<EnrichmentInfo> {
    ALL_ENRICHMENTS.iter().map(|e| e.info()).collect()
}

/// Run all enrichments on a recipe, continuing on failures.
///
/// Returns the enriched recipe and a list of any errors that occurred.
pub async fn run_all_enrichments(
    provider: &dyn LlmProvider,
    recipe: &RecipeContent,
) -> (RecipeContent, Vec<(String, EnrichmentError)>) {
    let mut current = recipe.clone();
    let mut errors = Vec::new();

    for enrichment in ALL_ENRICHMENTS {
        match enrichment.run(provider, &current).await {
            Ok(enriched) => {
                current = enriched;
            }
            Err(e) => {
                tracing::warn!(
                    enrichment = enrichment.enrichment_type(),
                    error = %e,
                    "Enrichment failed, continuing"
                );
                errors.push((enrichment.enrichment_type().to_string(), e));
            }
        }
    }

    (current, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_enrichment() {
        let enrichment = get_enrichment("normalize_ingredients");
        assert!(enrichment.is_some());
        assert_eq!(enrichment.unwrap().display_name(), "Normalize Ingredients");
    }

    #[test]
    fn test_get_enrichment_not_found() {
        let enrichment = get_enrichment("nonexistent");
        assert!(enrichment.is_none());
    }

    #[test]
    fn test_all_enrichment_info() {
        let info = all_enrichment_info();
        assert_eq!(info.len(), 4);
        assert!(info
            .iter()
            .any(|i| i.enrichment_type == "normalize_ingredients"));
        assert!(info.iter().any(|i| i.enrichment_type == "normalize_times"));
        assert!(info.iter().any(|i| i.enrichment_type == "add_nutrition"));
        assert!(info
            .iter()
            .any(|i| i.enrichment_type == "improve_instructions"));
    }

    #[test]
    fn test_all_enrichments_have_output_fields() {
        for enrichment in ALL_ENRICHMENTS {
            assert!(
                !enrichment.output_fields().is_empty(),
                "{} has no output fields",
                enrichment.enrichment_type()
            );
        }
    }
}
