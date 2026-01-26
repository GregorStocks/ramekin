//! Enrich step - enriches recipe data (stub that always fails).

use std::time::Instant;

use async_trait::async_trait;
use serde_json::json;

use crate::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};

/// Step that enriches recipe data using AI.
///
/// Currently a stub that always fails - enrichment is expected to be unreliable.
/// This step has `continues_on_failure: true` so the pipeline doesn't fail.
pub struct EnrichStep;

impl EnrichStep {
    /// Step name constant.
    pub const NAME: &'static str = "enrich";
}

#[async_trait]
impl PipelineStep for EnrichStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Enrich recipe with AI",
            continues_on_failure: true, // Enrichment failures don't fail the pipeline
        }
    }

    async fn execute(&self, _ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // TODO: Implement actual AI enrichment
        // For now, always fail (but continues_on_failure means pipeline continues)
        StepResult {
            success: false,
            output: json!({ "success": false }),
            error: Some("Enrichment not implemented".to_string()),
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: None, // Terminal step
        }
    }
}
