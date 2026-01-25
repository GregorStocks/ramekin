//! CLI FetchImages step - no-op that skips image fetching.

use std::time::Instant;

use async_trait::async_trait;
use ramekin_core::pipeline::steps::FetchImagesStepMeta;
use ramekin_core::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};
use serde_json::json;

/// CLI implementation of FetchImages - a no-op that returns empty photo_ids.
///
/// In CLI mode, we don't fetch images or store them in a database.
/// This step just passes through to save_recipe.
pub struct FetchImagesNoOp;

#[async_trait]
impl PipelineStep for FetchImagesNoOp {
    fn metadata(&self) -> StepMetadata {
        FetchImagesStepMeta::metadata()
    }

    async fn execute(&self, _ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        StepResult {
            success: true,
            output: json!({ "photo_ids": [], "skipped": true }),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("save_recipe".to_string()),
        }
    }
}
