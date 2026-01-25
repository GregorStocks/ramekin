//! Claude (Anthropic) LLM provider.

use super::{LlmError, LlmProvider};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Claude API provider.
#[derive(Debug)]
pub struct ClaudeProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl ClaudeProvider {
    /// Create a new ClaudeProvider with the given API key and model.
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }

    /// Create with the default model (claude-3-5-sonnet).
    #[allow(dead_code)]
    pub fn with_default_model(api_key: String) -> Self {
        Self::new(api_key, "claude-3-5-sonnet-20241022".to_string())
    }
}

/// Claude API request format.
#[derive(Debug, Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<ClaudeMessage>,
}

#[derive(Debug, Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

/// Claude API response format.
#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
    #[allow(dead_code)]
    #[serde(default)]
    error: Option<ClaudeApiError>,
}

#[derive(Debug, Deserialize)]
struct ClaudeContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeApiError {
    #[allow(dead_code)]
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

/// Error response from Claude API.
#[derive(Debug, Deserialize)]
struct ClaudeErrorResponse {
    error: ClaudeApiError,
}

#[async_trait]
impl LlmProvider for ClaudeProvider {
    async fn complete(&self, prompt: &str) -> Result<String, LlmError> {
        let request = ClaudeRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| LlmError::RequestFailed(e.to_string()))?;

        let status = response.status().as_u16();

        if status == 429 {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok());
            return Err(LlmError::RateLimited {
                retry_after_secs: retry_after,
            });
        }

        let body = response
            .text()
            .await
            .map_err(|e| LlmError::RequestFailed(e.to_string()))?;

        if status != 200 {
            // Try to parse error response
            if let Ok(error_response) = serde_json::from_str::<ClaudeErrorResponse>(&body) {
                return Err(LlmError::ApiError {
                    status,
                    message: error_response.error.message,
                });
            }
            return Err(LlmError::ApiError {
                status,
                message: body,
            });
        }

        let response: ClaudeResponse =
            serde_json::from_str(&body).map_err(|e| LlmError::ParseError(e.to_string()))?;

        // Extract text from the first text content block
        let text = response
            .content
            .into_iter()
            .find_map(|c| {
                if c.content_type == "text" {
                    c.text
                } else {
                    None
                }
            })
            .ok_or_else(|| LlmError::ParseError("No text content in response".to_string()))?;

        Ok(text)
    }

    fn provider_name(&self) -> &'static str {
        "claude"
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}
