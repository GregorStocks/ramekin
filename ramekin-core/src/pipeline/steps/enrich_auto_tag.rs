//! Enrich step - auto-tag recipes based on user's existing tags.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::json;

use crate::ai::{suggest_tags, AiClient};
use crate::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};

/// Step that automatically tags recipes based on user's existing tags.
///
/// This step uses AI to analyze recipe content and suggest relevant tags
/// from the user's existing tag list. It never creates new tags.
pub struct EnrichAutoTagStep {
    ai_client: Arc<dyn AiClient>,
    user_tags: Vec<String>,
}

impl EnrichAutoTagStep {
    /// Step name constant.
    pub const NAME: &'static str = "enrich_auto_tag";

    /// Create a new auto-tag step with the given AI client and user tags.
    pub fn new(ai_client: Arc<dyn AiClient>, user_tags: Vec<String>) -> Self {
        Self {
            ai_client,
            user_tags,
        }
    }
}

/// Output of the auto-tag step.
#[derive(Debug, Serialize)]
struct AutoTagOutput {
    suggested_tags: Vec<String>,
    cached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<crate::ai::Usage>,
}

#[async_trait]
impl PipelineStep for EnrichAutoTagStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Auto-tag recipe based on user's existing tags",
            continues_on_failure: false, // Errors should stop the pipeline
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Get recipe from extract_recipe output
        let extract_output = match ctx.outputs.get_output("extract_recipe") {
            Some(o) => o,
            None => {
                return StepResult {
                    step_name: Self::NAME.to_string(),
                    success: false,
                    output: json!({ "error": "No extract_recipe output found" }),
                    error: Some("No extract_recipe output found".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: Some("apply_auto_tags".to_string()),
                };
            }
        };

        let raw_recipe = match extract_output.get("raw_recipe") {
            Some(r) => r,
            None => {
                return StepResult {
                    step_name: Self::NAME.to_string(),
                    success: false,
                    output: json!({ "error": "No raw_recipe in extract output" }),
                    error: Some("No raw_recipe in extract output".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: Some("apply_auto_tags".to_string()),
                };
            }
        };

        // Extract recipe fields
        let title = raw_recipe
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let ingredients = raw_recipe
            .get("ingredients")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let instructions = raw_recipe
            .get("instructions")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Call the shared suggest_tags function
        let result = match suggest_tags(
            self.ai_client.as_ref(),
            title,
            ingredients,
            instructions,
            &self.user_tags,
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
                    next_step: Some("apply_auto_tags".to_string()),
                };
            }
        };

        let output = AutoTagOutput {
            suggested_tags: result.suggested_tags,
            cached: result.cached,
            usage: Some(result.usage),
        };

        StepResult {
            step_name: Self::NAME.to_string(),
            success: true,
            output: serde_json::to_value(output).unwrap_or(json!({})),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("apply_auto_tags".to_string()),
        }
    }
}
