//! ImproveInstructions enrichment.
//!
//! Improves recipe instructions for clarity and consistency.

use super::{Enrichment, EnrichmentError};
use crate::types::RecipeContent;
use async_trait::async_trait;

/// Enrichment that improves recipe instructions.
///
/// Reformats instructions for clarity, adds numbered steps,
/// and ensures consistent formatting.
#[derive(Debug, Clone, Copy)]
pub struct ImproveInstructions;

#[async_trait]
impl Enrichment for ImproveInstructions {
    fn enrichment_type(&self) -> &'static str {
        "improve_instructions"
    }

    fn display_name(&self) -> &'static str {
        "Improve Instructions"
    }

    fn description(&self) -> &'static str {
        "Reformat instructions for clarity with numbered steps"
    }

    fn output_fields(&self) -> &'static [&'static str] {
        &["instructions"]
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
            "ImproveInstructions enrichment not yet implemented".to_string(),
        ))
    }

    async fn run(
        &self,
        _provider: &dyn crate::llm::LlmProvider,
        _recipe: &RecipeContent,
    ) -> Result<RecipeContent, EnrichmentError> {
        Err(EnrichmentError::Validation(
            "ImproveInstructions enrichment not yet implemented".to_string(),
        ))
    }
}
