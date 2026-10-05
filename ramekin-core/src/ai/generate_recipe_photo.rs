//! Generate a recipe photo from structured recipe data.

use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use crate::ai::prompts::generate_recipe_photo::render_generate_recipe_photo_prompt;

use super::config::IMAGE_TIMEOUT_SECS;
use super::{AiConfig, AiError};

/// Prompt name for logging and future cache organization.
pub const GENERATE_RECIPE_PHOTO_PROMPT_NAME: &str = "generate_recipe_photo";

/// Result of generating a recipe photo.
#[derive(Debug, Clone)]
pub struct GenerateRecipePhotoResult {
    /// Image returned by the provider as a data URL.
    pub image_data_url: String,
    /// What the provider says the call cost in dollars (OpenRouter's
    /// `usage.cost`), if it says.
    pub cost: Option<f64>,
}

/// One generated image from OpenRouter's images endpoint.
#[derive(Debug, Deserialize)]
struct GeneratedImage {
    b64_json: String,
    #[serde(default = "default_media_type")]
    media_type: String,
}

fn default_media_type() -> String {
    "image/png".to_string()
}

#[derive(Debug, Deserialize)]
struct ImageUsage {
    #[serde(default)]
    cost: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ImageGenerationResponse {
    #[serde(default)]
    data: Vec<GeneratedImage>,
    #[serde(default)]
    usage: Option<ImageUsage>,
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
    // The images endpoint serves every image model, including those that
    // only make images (FLUX, Seedream, GPT Image), which chat/completions
    // refuses.
    let url = format!("{}/images", config.base_url.trim_end_matches('/'));

    tracing::debug!(
        prompt_name = GENERATE_RECIPE_PHOTO_PROMPT_NAME,
        model = %config.image_model,
        "Calling AI image generation API"
    );

    let response = Client::new()
        .post(url)
        .bearer_auth(&config.api_key)
        .json(&{
            let mut body = json!({
                "model": config.image_model,
                "prompt": prompt,
            });
            if let Some(quality) = &config.image_quality {
                body["quality"] = json!(quality);
            }
            body
        })
        .timeout(std::time::Duration::from_secs(
            config.request_timeout_secs.max(IMAGE_TIMEOUT_SECS),
        ))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                AiError::Timeout(format!("image generation: {e}"))
            } else {
                AiError::Api(e.to_string())
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<failed to read response body: {}>", e));
        let message = format!("Image generation request failed with {}: {}", status, body);
        // A content filter's refusal is about this recipe's prompt, not the
        // provider: OpenRouter passes it on as a client error saying so.
        if status.is_client_error() && body.to_lowercase().contains("blocked") {
            return Err(AiError::Refused(message));
        }
        return Err(AiError::Api(message));
    }

    let parsed: ImageGenerationResponse = response.json().await.map_err(|e| {
        // The timeout covers reading the body too, which a slow model's
        // image can outlast: that's the provider, not the answer.
        if e.is_timeout() {
            AiError::Timeout(format!("image generation: {e}"))
        } else {
            AiError::ParseError(format!("Failed to parse image response: {}", e))
        }
    })?;

    let cost = parsed.usage.and_then(|usage| usage.cost);
    let image = parsed.data.into_iter().next().ok_or_else(|| {
        AiError::ParseError("Image generation response did not include an image".to_string())
    })?;
    let image_data_url = format!("data:{};base64,{}", image.media_type, image.b64_json);

    Ok(GenerateRecipePhotoResult {
        image_data_url,
        cost,
    })
}
