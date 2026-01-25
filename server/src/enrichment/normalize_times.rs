//! NormalizeTimes enrichment.
//!
//! Standardizes prep_time, cook_time, and total_time fields to a consistent format.

use super::{Enrichment, EnrichmentError};
use crate::types::RecipeContent;
use async_trait::async_trait;

/// Enrichment that normalizes time fields.
///
/// Converts prep_time, cook_time, and total_time to a consistent format
/// (e.g., "15 minutes", "1 hour 30 minutes").
#[derive(Debug, Clone, Copy)]
pub struct NormalizeTimes;

#[async_trait]
impl Enrichment for NormalizeTimes {
    fn enrichment_type(&self) -> &'static str {
        "normalize_times"
    }

    fn display_name(&self) -> &'static str {
        "Normalize Times"
    }

    fn description(&self) -> &'static str {
        "Standardize prep, cook, and total times to a consistent format"
    }

    fn output_fields(&self) -> &'static [&'static str] {
        &["prep_time", "cook_time", "total_time"]
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
            "NormalizeTimes enrichment not yet implemented".to_string(),
        ))
    }

    async fn run(
        &self,
        _provider: &dyn crate::llm::LlmProvider,
        _recipe: &RecipeContent,
    ) -> Result<RecipeContent, EnrichmentError> {
        Err(EnrichmentError::Validation(
            "NormalizeTimes enrichment not yet implemented".to_string(),
        ))
    }
}
