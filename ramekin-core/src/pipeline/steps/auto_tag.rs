//! Normalize step - auto-tag recipes based on user's existing tags.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde::Serialize;

use crate::ai::{suggest_tags, AiClient};
use crate::pipeline::steps::{base_raw_recipe, normalize_step_failure};
use crate::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};

/// Step that automatically tags recipes based on user's existing tags.
///
/// This step uses AI to analyze recipe content and suggest relevant tags
/// from the user's existing tag list. It never creates new tags.
pub struct AutoTagStep {
    ai_client: Arc<dyn AiClient>,
    user_tags: Vec<String>,
}

impl AutoTagStep {
    /// Step name constant.
    pub const NAME: &'static str = "auto_tag";

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
impl PipelineStep for AutoTagStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Auto-tag recipe based on user's existing tags",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();
        let raw_recipe = match base_raw_recipe(ctx).await {
            Ok(r) => r,
            Err(e) => return normalize_step_failure(Self::NAME, start, e),
        };

        let result = match suggest_tags(
            self.ai_client.as_ref(),
            &raw_recipe.title,
            &raw_recipe.ingredients,
            &raw_recipe.instructions,
            &self.user_tags,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                return normalize_step_failure(Self::NAME, start, format!("AI call failed: {e}"))
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
            output: serde_json::to_value(output).expect("AutoTagOutput serializes to JSON"),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: None,
        }
    }
}
