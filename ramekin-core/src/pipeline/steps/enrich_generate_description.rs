//! Enrich step - generate recipe descriptions during ingestion.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::json;

use crate::ai::{generate_description, AiClient, Usage};
use crate::pipeline::{
    deserialize_required_output_field, step_after_scrape_auto_applied_ai_step, PipelineStep,
    StepContext, StepMetadata, StepResult,
};
use crate::types::RawRecipe;

/// Step that generates concise menu-style recipe descriptions.
pub struct EnrichGenerateDescriptionStep {
    ai_client: Arc<dyn AiClient>,
}

impl EnrichGenerateDescriptionStep {
    pub const NAME: &'static str = "enrich_generate_description";

    pub fn new(ai_client: Arc<dyn AiClient>) -> Self {
        Self { ai_client }
    }
}

#[derive(Debug, Serialize)]
struct GenerateDescriptionOutput {
    original_description: Option<String>,
    generated_description: String,
    changed: bool,
    cached: bool,
    usage: Usage,
}

async fn title_for_description(ctx: &StepContext<'_>, raw_title: &str) -> String {
    ctx.outputs
        .get_output(EnrichNormalizeTitleStepName::NAME)
        .await
        .and_then(|o| {
            let changed = o.get("changed").and_then(|v| v.as_bool()).unwrap_or(false);
            if !changed {
                return None;
            }
            o.get("normalized_title")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| raw_title.to_string())
}

struct EnrichNormalizeTitleStepName;

impl EnrichNormalizeTitleStepName {
    const NAME: &'static str = "enrich_normalize_title";
}

#[async_trait]
impl PipelineStep for EnrichGenerateDescriptionStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Generate recipe description",
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

        let raw_title = raw_recipe.title.as_str();
        let title = title_for_description(ctx, raw_title).await;
        let original_description = raw_recipe.description.clone();
        let ingredients = raw_recipe.ingredients.as_str();
        let instructions = raw_recipe.instructions.as_str();

        let result =
            match generate_description(self.ai_client.as_ref(), &title, ingredients, instructions)
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

        let generated_description = result.description.trim().to_string();
        let output = GenerateDescriptionOutput {
            original_description: original_description.clone(),
            changed: !generated_description.is_empty()
                && original_description.as_deref() != Some(&generated_description),
            generated_description,
            cached: result.cached,
            usage: result.usage,
        };

        StepResult {
            step_name: Self::NAME.to_string(),
            success: true,
            output: serde_json::to_value(output)
                .expect("GenerateDescriptionOutput serializes to JSON"),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: step_after_scrape_auto_applied_ai_step(Self::NAME).map(str::to_string),
        }
    }
}
