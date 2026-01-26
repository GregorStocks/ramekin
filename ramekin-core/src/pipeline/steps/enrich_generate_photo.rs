//! Enrich step - generate AI photo (stub that always fails).

use std::time::Instant;

use async_trait::async_trait;
use serde_json::json;

use crate::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};

/// Step that generates an AI photo if the recipe has no photo.
///
/// Currently a stub that always fails - enrichment is expected to be unreliable.
/// This step has `continues_on_failure: true` so the pipeline doesn't fail.
pub struct EnrichGeneratePhotoStep;

impl EnrichGeneratePhotoStep {
    /// Step name constant.
    pub const NAME: &'static str = "enrich_generate_photo";
}

#[async_trait]
impl PipelineStep for EnrichGeneratePhotoStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Generate AI photo if recipe has no photo",
            continues_on_failure: true,
        }
    }

    async fn execute(&self, _ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // TODO: Implement actual AI photo generation
        // For now, always fail (but continues_on_failure means pipeline continues)
        StepResult {
            success: false,
            output: json!({ "success": false }),
            error: Some("Photo generation not implemented".to_string()),
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: None, // Terminal step
        }
    }
}
