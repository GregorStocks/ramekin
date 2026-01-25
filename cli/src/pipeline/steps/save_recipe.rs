//! CLI SaveRecipe step - saves recipe to file.

use std::time::Instant;

use async_trait::async_trait;
use chrono::Utc;
use ramekin_core::pipeline::steps::SaveRecipeStepMeta;
use ramekin_core::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};
use serde_json::json;

/// CLI implementation of SaveRecipe - saves recipe data to file.
///
/// In CLI mode, we don't save to a database. Instead, we just pass through
/// the raw_recipe data with a timestamp, and the FileOutputStore handles
/// writing it to output.json.
pub struct SaveRecipeStep;

#[async_trait]
impl PipelineStep for SaveRecipeStep {
    fn metadata(&self) -> StepMetadata {
        SaveRecipeStepMeta::metadata()
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Get raw_recipe from extract_recipe output
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

        let raw_recipe = match extract_output.get("raw_recipe") {
            Some(r) => r.clone(),
            None => {
                return StepResult {
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("No raw_recipe in extract_recipe output".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        StepResult {
            success: true,
            output: json!({
                "raw_recipe": raw_recipe,
                "saved_at": Utc::now().to_rfc3339()
            }),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("enrich".to_string()),
        }
    }
}
