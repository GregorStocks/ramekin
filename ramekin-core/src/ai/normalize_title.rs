//! Normalize a recipe title by stripping clickbait / marketing language.

use serde::Deserialize;

use crate::ai::prompts::normalize_title::{
    render_normalize_title_prompt, NORMALIZE_TITLE_PROMPT_NAME,
};
use crate::ai::{complete_json, AiClient, AiError, ChatMessage, ChatRequest, Usage};

#[derive(Debug, Deserialize)]
struct NormalizeTitleResponse {
    normalized_title: String,
}

/// Result of a title-normalization call.
pub struct NormalizeTitleResult {
    pub normalized_title: String,
    pub cached: bool,
    pub usage: Usage,
}

/// De-clickbait a recipe title via the AI client.
///
/// The full ingredient list and instructions are passed to the model so it can
/// disambiguate obscure dish names and confirm what the recipe actually is.
pub async fn normalize_title(
    ai_client: &dyn AiClient,
    title: &str,
    ingredients: &str,
    instructions: &str,
) -> Result<NormalizeTitleResult, AiError> {
    let prompt = render_normalize_title_prompt(title, ingredients, instructions);
    let request = ChatRequest {
        messages: vec![ChatMessage::user(prompt)],
        json_response: true,
        // Generous budget: reasoning models spend output tokens on hidden
        // thinking before the short JSON answer, and a too-small cap truncates
        // the answer itself.
        max_tokens: Some(4096),
        temperature: Some(0.0),
    };

    let (parsed, response): (NormalizeTitleResponse, _) =
        complete_json(ai_client, NORMALIZE_TITLE_PROMPT_NAME, &request).await?;

    let normalized = parsed.normalized_title.trim().to_string();
    let normalized = if normalized.is_empty() {
        title.trim().to_string()
    } else {
        normalized
    };

    Ok(NormalizeTitleResult {
        normalized_title: normalized,
        cached: response.cached,
        usage: response.usage,
    })
}
