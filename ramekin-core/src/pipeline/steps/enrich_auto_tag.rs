//! Enrich step - auto-tag recipes based on user's existing tags.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ai::prompts::auto_tag::{render_auto_tag_prompt, AUTO_TAG_PROMPT_NAME};
use crate::ai::{AiClient, ChatMessage, ChatRequest};
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

/// Response format from the AI.
#[derive(Debug, Deserialize)]
struct AutoTagResponse {
    suggested_tags: Vec<String>,
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

        // If no user tags, return success with empty suggestions
        // This is a valid case for CLI where there's no user context
        if self.user_tags.is_empty() {
            return StepResult {
                step_name: Self::NAME.to_string(),
                success: true,
                output: json!(AutoTagOutput {
                    suggested_tags: vec![],
                    cached: false,
                    usage: None,
                }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: Some("enrich_generate_photo".to_string()),
            };
        }

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
                    next_step: Some("enrich_generate_photo".to_string()),
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
                    next_step: Some("enrich_generate_photo".to_string()),
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

        // Build the prompt
        let prompt = render_auto_tag_prompt(title, ingredients, instructions, &self.user_tags);

        // Build the AI request
        let request = ChatRequest {
            messages: vec![ChatMessage::user(prompt)],
            json_response: true,
            max_tokens: Some(256),
            temperature: Some(0.3), // Lower temperature for more deterministic results
        };

        // Call the AI
        let response = match self.ai_client.complete(AUTO_TAG_PROMPT_NAME, request).await {
            Ok(r) => r,
            Err(e) => {
                return StepResult {
                    step_name: Self::NAME.to_string(),
                    success: false,
                    output: json!({ "error": format!("AI call failed: {}", e) }),
                    error: Some(format!("AI call failed: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: Some("enrich_generate_photo".to_string()),
                };
            }
        };

        // Parse the AI response
        let ai_response: AutoTagResponse = match serde_json::from_str(&response.content) {
            Ok(r) => r,
            Err(e) => {
                return StepResult {
                    step_name: Self::NAME.to_string(),
                    success: false,
                    output: json!({
                        "error": format!("Failed to parse AI response: {}", e),
                        "raw_response": response.content,
                    }),
                    error: Some(format!("Failed to parse AI response: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: Some("enrich_generate_photo".to_string()),
                };
            }
        };

        // Filter to only valid existing tags (case-insensitive match, preserving user's casing)
        let valid_tags: Vec<String> = ai_response
            .suggested_tags
            .into_iter()
            .filter_map(|suggested| {
                self.user_tags
                    .iter()
                    .find(|ut| ut.eq_ignore_ascii_case(&suggested))
                    .cloned()
            })
            .collect();

        let output = AutoTagOutput {
            suggested_tags: valid_tags,
            cached: response.cached,
            usage: Some(response.usage),
        };

        StepResult {
            step_name: Self::NAME.to_string(),
            success: true,
            output: serde_json::to_value(output).unwrap_or(json!({})),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("enrich_generate_photo".to_string()),
        }
    }
}
