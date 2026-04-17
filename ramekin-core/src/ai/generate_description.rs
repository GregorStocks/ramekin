//! Generate a concise menu-style description for a recipe.

use serde::Deserialize;

use crate::ai::prompts::generate_description::{
    render_generate_description_prompt, GENERATE_DESCRIPTION_PROMPT_NAME,
};
use crate::ai::{AiClient, AiError, ChatMessage, ChatRequest, Usage};

#[derive(Debug, Deserialize)]
struct GenerateDescriptionResponse {
    description: String,
}

/// Result of a description-generation call.
pub struct GenerateDescriptionResult {
    pub description: String,
    pub cached: bool,
    pub usage: Usage,
}

/// Generate a concise menu-style description for a recipe via the AI client.
///
/// The full ingredient list and instructions are passed to the model so it can
/// write an accurate description of what the dish actually is.
pub async fn generate_description(
    ai_client: &dyn AiClient,
    title: &str,
    ingredients: &str,
    instructions: &str,
) -> Result<GenerateDescriptionResult, AiError> {
    let prompt = render_generate_description_prompt(title, ingredients, instructions);
    let request = ChatRequest {
        messages: vec![ChatMessage::user(prompt)],
        json_response: true,
        max_tokens: Some(256),
        temperature: Some(0.0),
    };

    let response = ai_client
        .complete(GENERATE_DESCRIPTION_PROMPT_NAME, request)
        .await?;

    let parsed: GenerateDescriptionResponse =
        serde_json::from_str(&response.content).map_err(|e| {
            AiError::ParseError(format!(
                "Failed to parse generate-description response: {}",
                e
            ))
        })?;

    let description = parsed.description.trim().to_string();

    Ok(GenerateDescriptionResult {
        description,
        cached: response.cached,
        usage: response.usage,
    })
}
