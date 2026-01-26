//! Enrich step - normalize ingredients (stub that always fails).

use std::time::Instant;

use async_trait::async_trait;
use serde_json::json;

use crate::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};

/// Step that normalizes ingredient text using AI.
///
/// Currently a stub that always fails - enrichment is expected to be unreliable.
/// This step has `continues_on_failure: true` so the pipeline doesn't fail.
pub struct EnrichNormalizeIngredientsStep;

impl EnrichNormalizeIngredientsStep {
    /// Step name constant.
    pub const NAME: &'static str = "enrich_normalize_ingredients";
}

#[async_trait]
impl PipelineStep for EnrichNormalizeIngredientsStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Normalize ingredient text with AI",
            continues_on_failure: true,
        }
    }

    async fn execute(&self, _ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // TODO: Implement actual AI ingredient normalization
        // For now, always fail (but continues_on_failure means pipeline continues)
        StepResult {
            success: false,
            output: json!({ "success": false }),
            error: Some("Ingredient normalization not implemented".to_string()),
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("enrich_auto_tag".to_string()),
        }
    }
}
