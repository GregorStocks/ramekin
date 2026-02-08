//! Custom enrichment: apply user-specified changes to recipes via AI.

use crate::ai::prompts::custom_enrich::{
    render_custom_enrich_system_prompt, render_custom_enrich_user_prompt, CUSTOM_ENRICH_PROMPT_NAME,
};
use crate::ai::{AiClient, AiError, ChatMessage, ChatRequest, Usage};

/// Result of custom enrichment.
pub struct CustomEnrichResult {
    /// The modified recipe as a JSON string (to be deserialized by the caller).
    pub recipe_json: String,
    pub cached: bool,
    pub usage: Usage,
}

/// Apply a user-specified change to a recipe using AI.
///
/// Takes the recipe as a JSON string and the user's instruction describing
/// what change to make. Returns the complete modified recipe as a JSON string.
pub async fn custom_enrich(
    ai_client: &dyn AiClient,
    recipe_json: &str,
    instruction: &str,
) -> Result<CustomEnrichResult, AiError> {
    let system_prompt = render_custom_enrich_system_prompt();
    let user_prompt = render_custom_enrich_user_prompt(recipe_json, instruction);

    let request = ChatRequest {
        messages: vec![
            ChatMessage::system(system_prompt),
            ChatMessage::user(user_prompt),
        ],
        json_response: true,
        max_tokens: Some(4096),
        temperature: Some(0.7),
    };

    let response = ai_client
        .complete(CUSTOM_ENRICH_PROMPT_NAME, request)
        .await?;

    Ok(CustomEnrichResult {
        recipe_json: response.content,
        cached: response.cached,
        usage: response.usage,
    })
}
