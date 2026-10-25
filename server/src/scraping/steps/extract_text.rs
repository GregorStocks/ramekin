use std::time::Instant;

use async_trait::async_trait;

use ramekin_core::ai::text_extract::extract_recipe_from_text;
use ramekin_core::ai::{AiConfig, CachingAiClient};
use ramekin_core::pipeline::steps::ExtractRecipeStep;
use ramekin_core::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};
use ramekin_core::{ExtractRecipeOutput, ExtractionMethod};

use super::super::jobs::SOURCE_TEXT_STEP;
use super::helpers::step_failure;

/// `extract_recipe` for a recipe created from pasted text: reads the text
/// again with the text extraction model, as the import did.
pub struct ExtractTextStep;

#[async_trait]
impl PipelineStep for ExtractTextStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: ExtractRecipeStep::NAME,
            description: "Extract the recipe from its pasted text",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();
        let fail = |error: String| step_failure(ExtractRecipeStep::NAME, start, error);
        let Some(text) = ctx
            .outputs
            .get_output(SOURCE_TEXT_STEP)
            .await
            .and_then(|output| output["text"].as_str().map(str::to_string))
        else {
            return fail("source_text output not found".to_string());
        };
        let client = match AiConfig::from_env() {
            Ok(config) => CachingAiClient::new(config.for_extraction()),
            Err(e) => return fail(format!("AI is not configured: {e}")),
        };
        let extracted = match extract_recipe_from_text(&client, &text).await {
            Ok(extracted) => extracted,
            Err(e) => return fail(format!("Text extraction failed: {e}")),
        };
        let output = ExtractRecipeOutput {
            raw_recipe: extracted.raw_recipe,
            method_used: ExtractionMethod::Text,
            all_attempts: Vec::new(),
        };
        StepResult {
            step_name: ExtractRecipeStep::NAME.to_string(),
            success: true,
            output: serde_json::to_value(output).expect("ExtractRecipeOutput serializes to JSON"),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: None,
        }
    }
}
