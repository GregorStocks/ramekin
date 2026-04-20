//! Generate a recipe photo from structured recipe data.

use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use crate::ai::prompts::generate_recipe_photo::render_generate_recipe_photo_prompt;

use super::{AiConfig, AiError};

/// Prompt name for logging and future cache organization.
pub const GENERATE_RECIPE_PHOTO_PROMPT_NAME: &str = "generate_recipe_photo";

/// Result of generating a recipe photo.
#[derive(Debug, Clone)]
pub struct GenerateRecipePhotoResult {
    /// Image returned by the provider as a data URL.
    pub image_data_url: String,
}

#[derive(Debug, Deserialize)]
struct ImageUrlPayload {
    url: String,
}

#[derive(Debug, Deserialize)]
struct GeneratedImage {
    image_url: ImageUrlPayload,
}

#[derive(Debug, Deserialize)]
struct AssistantMessage {
    #[serde(default)]
    images: Vec<GeneratedImage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: AssistantMessage,
}

#[derive(Debug, Deserialize)]
struct ImageCompletionResponse {
    choices: Vec<Choice>,
}

/// Generate a recipe photo using the configured AI image model.
pub async fn generate_recipe_photo(
    config: &AiConfig,
    title: &str,
    description: Option<&str>,
    ingredients: &str,
    instructions: &str,
) -> Result<GenerateRecipePhotoResult, AiError> {
    let prompt = render_generate_recipe_photo_prompt(title, description, ingredients, instructions);
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    tracing::debug!(
        prompt_name = GENERATE_RECIPE_PHOTO_PROMPT_NAME,
        model = %config.image_model,
        "Calling AI image generation API"
    );

    let response = Client::new()
        .post(url)
        .bearer_auth(&config.api_key)
        .json(&json!({
            "model": config.image_model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt,
                }
            ],
            "modalities": ["image", "text"],
            "stream": false
        }))
        .timeout(std::time::Duration::from_secs(config.request_timeout_secs))
        .send()
        .await
        .map_err(|e| AiError::Api(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AiError::Api(format!(
            "Image generation request failed with {}: {}",
            status, body
        )));
    }

    let parsed: ImageCompletionResponse = response
        .json()
        .await
        .map_err(|e| AiError::ParseError(format!("Failed to parse image response: {}", e)))?;

    let image_data_url = parsed
        .choices
        .into_iter()
        .next()
        .and_then(|choice| choice.message.images.into_iter().next())
        .map(|image| image.image_url.url)
        .ok_or_else(|| {
            AiError::ParseError("Image generation response did not include an image".to_string())
        })?;

    Ok(GenerateRecipePhotoResult { image_data_url })
}
