//! Normalize step - generate recipe descriptions.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde::Serialize;

use crate::ai::{generate_description, AiClient, Usage};
use crate::pipeline::steps::{base_raw_recipe, step_failure, NormalizeTitleStep};
use crate::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};

/// Step that generates concise menu-style recipe descriptions.
pub struct GenerateDescriptionStep {
    ai_client: Arc<dyn AiClient>,
}

impl GenerateDescriptionStep {
    pub const NAME: &'static str = "generate_description";

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

/// The normalized title when this job normalized it, else the recipe's own.
async fn title_for_description(ctx: &StepContext<'_>, raw_title: &str) -> String {
    ctx.outputs
        .get_output(NormalizeTitleStep::NAME)
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

#[async_trait]
impl PipelineStep for GenerateDescriptionStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Generate recipe description",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();
        let raw_recipe = match base_raw_recipe(ctx).await {
            Ok(r) => r,
            Err(e) => return step_failure(Self::NAME, start, e),
        };

        let title = title_for_description(ctx, &raw_recipe.title).await;
        let original_description = raw_recipe.description.clone();
        let result = match generate_description(
            self.ai_client.as_ref(),
            &title,
            &raw_recipe.ingredients,
            &raw_recipe.instructions,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => return step_failure(Self::NAME, start, format!("AI call failed: {e}")),
        };

        let generated_description = result.description.trim().to_string();
        let output = GenerateDescriptionOutput {
            changed: !generated_description.is_empty()
                && original_description.as_deref() != Some(&generated_description),
            original_description,
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
            next_step: None,
        }
    }
}
