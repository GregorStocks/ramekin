//! Generate a concise menu-style description for a recipe.

use serde::Deserialize;

use crate::ai::prompts::generate_description::{
    render_generate_description_prompt, GENERATE_DESCRIPTION_PROMPT_NAME,
};
use crate::ai::{
    complete_json, AiClient, AiError, ChatMessage, ChatRequest, Usage,
    SHORT_JSON_ANSWER_MAX_TOKENS,
};

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
        max_tokens: Some(SHORT_JSON_ANSWER_MAX_TOKENS),
        temperature: Some(0.0),
    };

    let (parsed, response): (GenerateDescriptionResponse, _) =
        complete_json(ai_client, GENERATE_DESCRIPTION_PROMPT_NAME, &request).await?;

    let description = parsed.description.trim().to_string();

    Ok(GenerateDescriptionResult {
        description,
        cached: response.cached,
        usage: response.usage,
    })
}
