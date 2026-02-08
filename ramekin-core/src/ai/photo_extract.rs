//! Recipe extraction from photos using vision AI.

use serde::Deserialize;

use crate::ai::prompts::photo_extract::{render_photo_extract_prompt, PHOTO_EXTRACT_PROMPT_NAME};
use crate::ai::{AiClient, AiError, ChatMessage, ChatRequest, ImageData, Usage};
use crate::types::RawRecipe;

#[derive(Debug, Deserialize)]
struct PhotoExtractResponse {
    title: String,
    #[serde(default)]
    description: Option<String>,
    ingredients: String,
    instructions: String,
    #[serde(default)]
    servings: Option<String>,
    #[serde(default)]
    prep_time: Option<String>,
    #[serde(default)]
    cook_time: Option<String>,
    #[serde(default)]
    total_time: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

pub struct PhotoExtractResult {
    pub raw_recipe: RawRecipe,
    pub cached: bool,
    pub usage: Usage,
}

pub async fn extract_recipe_from_photos(
    ai_client: &dyn AiClient,
    images: Vec<ImageData>,
) -> Result<PhotoExtractResult, AiError> {
    let prompt = render_photo_extract_prompt();
    let request = ChatRequest {
        messages: vec![ChatMessage::user_with_images(prompt, images)],
        json_response: true,
        max_tokens: Some(4096),
        temperature: Some(0.1),
    };

    let response = ai_client
        .complete(PHOTO_EXTRACT_PROMPT_NAME, request)
        .await?;

    let extracted: PhotoExtractResponse = serde_json::from_str(&response.content).map_err(|e| {
        AiError::ParseError(format!("Failed to parse photo extraction response: {}", e))
    })?;

    let raw_recipe = RawRecipe {
        title: extracted.title,
        description: extracted.description,
        ingredients: extracted.ingredients,
        instructions: extracted.instructions,
        image_urls: vec![],
        source_url: None,
        source_name: None,
        servings: extracted.servings,
        prep_time: extracted.prep_time,
        cook_time: extracted.cook_time,
        total_time: extracted.total_time,
        rating: None,
        difficulty: None,
        nutritional_info: None,
        notes: extracted.notes,
        categories: None,
    };

    Ok(PhotoExtractResult {
        raw_recipe,
        cached: response.cached,
        usage: response.usage,
    })
}
