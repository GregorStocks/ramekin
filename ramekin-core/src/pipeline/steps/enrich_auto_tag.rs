//! Enrich step - auto-tag recipes (stub that always fails).

use std::time::Instant;

use async_trait::async_trait;
use serde_json::json;

use crate::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};

/// Step that automatically tags recipes based on user's existing tags.
///
/// Currently a stub that always fails - enrichment is expected to be unreliable.
/// This step has `continues_on_failure: true` so the pipeline doesn't fail.
pub struct EnrichAutoTagStep;

impl EnrichAutoTagStep {
    /// Step name constant.
    pub const NAME: &'static str = "enrich_auto_tag";
}

#[async_trait]
impl PipelineStep for EnrichAutoTagStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Auto-tag recipe based on user's existing tags",
            continues_on_failure: true,
        }
    }

    async fn execute(&self, _ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // TODO: Implement actual AI auto-tagging
        // For now, always fail (but continues_on_failure means pipeline continues)
        StepResult {
            step_name: Self::NAME.to_string(),
            success: false,
            output: json!({ "success": false }),
            error: Some("Auto-tagging not implemented".to_string()),
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("enrich_generate_photo".to_string()),
        }
    }
}
