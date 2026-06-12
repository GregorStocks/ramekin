//! Custom enrichment: apply user-specified changes to recipes via AI.

use crate::ai::prompts::custom_enrich::{
    render_custom_enrich_system_prompt, render_custom_enrich_user_prompt, CUSTOM_ENRICH_PROMPT_NAME,
};
use crate::ai::{complete_json, AiClient, AiError, ChatMessage, ChatRequest, ImageData, Usage};

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
    images: Vec<ImageData>,
) -> Result<CustomEnrichResult, AiError> {
    let system_prompt = render_custom_enrich_system_prompt();
    let user_prompt = render_custom_enrich_user_prompt(recipe_json, instruction);
    let user_message = if images.is_empty() {
        ChatMessage::user(user_prompt)
    } else {
        ChatMessage::user_with_images(user_prompt, images)
    };

    let request = ChatRequest {
        messages: vec![ChatMessage::system(system_prompt), user_message],
        json_response: true,
        max_tokens: Some(4096),
        temperature: Some(0.7),
    };

    // The recipe shape is the caller's concern, but validating that the content
    // is well-formed JSON here keeps truncated responses out of the cache.
    let (_, response): (serde_json::Value, _) =
        complete_json(ai_client, CUSTOM_ENRICH_PROMPT_NAME, request).await?;

    Ok(CustomEnrichResult {
        recipe_json: response.content,
        cached: response.cached,
        usage: response.usage,
    })
}
