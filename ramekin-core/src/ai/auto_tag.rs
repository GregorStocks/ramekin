//! Auto-tag functionality for suggesting recipe tags.

use serde::Deserialize;

use crate::ai::prompts::auto_tag::{render_auto_tag_prompt, AUTO_TAG_PROMPT_NAME};
use crate::ai::{AiClient, AiError, ChatMessage, ChatRequest, Usage};

/// Response format from the AI.
#[derive(Debug, Deserialize)]
struct AutoTagResponse {
    suggested_tags: Vec<String>,
}

/// Result of auto-tag suggestion.
pub struct AutoTagResult {
    pub suggested_tags: Vec<String>,
    pub cached: bool,
    pub usage: Usage,
}

/// Suggest tags for a recipe based on user's existing tags.
///
/// Returns tags from `user_tags` that match the recipe content.
/// Never creates new tags - only suggests from the provided list.
pub async fn suggest_tags(
    ai_client: &dyn AiClient,
    title: &str,
    ingredients: &str,
    instructions: &str,
    user_tags: &[String],
) -> Result<AutoTagResult, AiError> {
    // If no user tags, return empty
    if user_tags.is_empty() {
        return Ok(AutoTagResult {
            suggested_tags: vec![],
            cached: false,
            usage: Usage::default(),
        });
    }

    let prompt = render_auto_tag_prompt(title, ingredients, instructions, user_tags);
    let request = ChatRequest {
        messages: vec![ChatMessage::user(prompt)],
        json_response: true,
        max_tokens: Some(256),
        temperature: Some(0.3),
    };

    let response = ai_client.complete(AUTO_TAG_PROMPT_NAME, request).await?;

    let ai_response: AutoTagResponse = serde_json::from_str(&response.content)
        .map_err(|e| AiError::ParseError(format!("Failed to parse auto-tag response: {}", e)))?;

    // Filter to valid existing tags (case-insensitive, preserve user's casing)
    let valid_tags: Vec<String> = ai_response
        .suggested_tags
        .into_iter()
        .filter_map(|suggested| {
            user_tags
                .iter()
                .find(|ut| ut.eq_ignore_ascii_case(&suggested))
                .cloned()
        })
        .collect();

    Ok(AutoTagResult {
        suggested_tags: valid_tags,
        cached: response.cached,
        usage: response.usage,
    })
}
