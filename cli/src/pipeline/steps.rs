//! CLI-specific pipeline step implementations.
//!
//! These implement the `PipelineStep` trait for steps that need CLI-specific behavior
//! (primarily around file I/O instead of database operations).

use std::time::Instant;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;

use ramekin_core::pipeline::{
    steps::{FetchImagesStepMeta, SaveRecipeStepMeta},
    PipelineStep, StepContext, StepMetadata, StepResult,
};
use ramekin_core::ExtractRecipeOutput;

/// CLI implementation of FetchImages step.
///
/// Currently a pass-through that doesn't actually fetch images -
/// image fetching is not yet implemented in the CLI.
pub struct FetchImagesStep;

#[async_trait]
impl PipelineStep for FetchImagesStep {
    fn metadata(&self) -> StepMetadata {
        FetchImagesStepMeta::metadata()
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Get extract output to pass through
        let extract_output = ctx.outputs.get_output("extract_recipe");
        if extract_output.is_none() {
            return StepResult {
                success: false,
                output: serde_json::Value::Null,
                error: Some("extract_recipe output not found".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: None,
            };
        }

        // Pass through - image fetching not implemented in CLI
        StepResult {
            success: true,
            output: json!({
                "images_fetched": 0,
                "skipped": true,
                "reason": "Image fetching not implemented in CLI"
            }),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("save_recipe".to_string()),
        }
    }
}

/// CLI implementation of SaveRecipe step.
///
/// Saves the extracted recipe to the output directory as JSON.
pub struct SaveRecipeStep;

#[async_trait]
impl PipelineStep for SaveRecipeStep {
    fn metadata(&self) -> StepMetadata {
        SaveRecipeStepMeta::metadata()
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Get extract output
        let extract_output = match ctx.outputs.get_output("extract_recipe") {
            Some(o) => o,
            None => {
                return StepResult {
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("extract_recipe output not found".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Parse the extract output
        let extract_data: ExtractRecipeOutput = match serde_json::from_value(extract_output) {
            Ok(d) => d,
            Err(e) => {
                return StepResult {
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some(format!("Failed to parse extract output: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Create save output
        let save_output = ramekin_core::SaveRecipeOutput {
            raw_recipe: extract_data.raw_recipe,
            saved_at: Utc::now().to_rfc3339(),
        };

        StepResult {
            success: true,
            output: serde_json::to_value(&save_output).unwrap_or_default(),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("enrich".to_string()),
        }
    }
}
