//! Enrich step - normalize recipe titles during ingestion.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::json;

use crate::ai::{normalize_title, AiClient, Usage};
use crate::pipeline::{
    deserialize_required_output_field, step_after_scrape_auto_applied_ai_step, PipelineStep,
    StepContext, StepMetadata, StepResult,
};
use crate::types::RawRecipe;

/// Step that normalizes recipe titles using the shared AI title normalizer.
pub struct EnrichNormalizeTitleStep {
    ai_client: Arc<dyn AiClient>,
}

impl EnrichNormalizeTitleStep {
    pub const NAME: &'static str = "enrich_normalize_title";

    pub fn new(ai_client: Arc<dyn AiClient>) -> Self {
        Self { ai_client }
    }
}

#[derive(Debug, Serialize)]
struct NormalizeTitleOutput {
    original_title: String,
    normalized_title: String,
    changed: bool,
    cached: bool,
    usage: Usage,
}

#[async_trait]
impl PipelineStep for EnrichNormalizeTitleStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Normalize recipe title",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        let extract_output = match ctx.outputs.get_output("extract_recipe").await {
            Some(o) => o,
            None => {
                return StepResult {
                    step_name: Self::NAME.to_string(),
                    success: false,
                    output: json!({ "error": "No extract_recipe output found" }),
                    error: Some("No extract_recipe output found".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: step_after_scrape_auto_applied_ai_step(Self::NAME)
                        .map(str::to_string),
                };
            }
        };

        let raw_recipe: RawRecipe = match deserialize_required_output_field(
            &extract_output,
            "extract_recipe",
            "raw_recipe",
        ) {
            Ok(r) => r,
            Err(e) => {
                return StepResult {
                    step_name: Self::NAME.to_string(),
                    success: false,
                    output: json!({ "error": e }),
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: step_after_scrape_auto_applied_ai_step(Self::NAME)
                        .map(str::to_string),
                };
            }
        };

        let title = raw_recipe.title.as_str();
        let ingredients = raw_recipe.ingredients.as_str();
        let instructions = raw_recipe.instructions.as_str();

        let result = match normalize_title(
            self.ai_client.as_ref(),
            title,
            ingredients,
            instructions,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                return StepResult {
                    step_name: Self::NAME.to_string(),
                    success: false,
                    output: json!({ "error": format!("AI call failed: {}", e) }),
                    error: Some(format!("AI call failed: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: step_after_scrape_auto_applied_ai_step(Self::NAME)
                        .map(str::to_string),
                };
            }
        };

        let normalized_title = result.normalized_title.trim().to_string();
        let output = NormalizeTitleOutput {
            original_title: title.to_string(),
            changed: !normalized_title.is_empty() && normalized_title != title,
            normalized_title,
            cached: result.cached,
            usage: result.usage,
        };

        StepResult {
            step_name: Self::NAME.to_string(),
            success: true,
            output: serde_json::to_value(output).expect("NormalizeTitleOutput serializes to JSON"),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: step_after_scrape_auto_applied_ai_step(Self::NAME).map(str::to_string),
        }
    }
}
