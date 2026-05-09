//! AI client implementation using OpenRouter (OpenAI-compatible API).

use async_trait::async_trait;
use openai_api_rs::v1::api::OpenAIClient;
use openai_api_rs::v1::chat_completion::chat_completion::ChatCompletionRequest;
use openai_api_rs::v1::chat_completion::{
    ChatCompletionMessage, Content, ContentType, ImageUrl, ImageUrlType, MessageRole,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::time::Instant;

use super::cache::{AiCache, CacheKey};
use super::config::AiConfig;
use super::types::{ChatMessage, ChatRequest, ChatResponse, Role, Usage};

#[derive(Error, Debug)]
pub enum AiError {
    #[error("API error: {0}")]
    Api(String),

    #[error("Failed to parse response: {0}")]
    ParseError(String),

    #[error("Configuration error: {0}")]
    Config(#[from] super::config::ConfigError),
}

/// Trait for AI clients.
#[async_trait]
pub trait AiClient: Send + Sync {
    /// Complete a chat request.
    ///
    /// The `prompt_name` is used for cache organization. Cache invalidation happens
    /// automatically based on the content hash of the messages.
    async fn complete(
        &self,
        prompt_name: &str,
        request: ChatRequest,
    ) -> Result<ChatResponse, AiError>;
}

/// AI client with caching and rate limiting, using OpenRouter.
pub struct CachingAiClient {
    client: OpenAIClient,
    cache: AiCache,
    config: AiConfig,
    last_request: Arc<Mutex<Option<Instant>>>,
}

impl CachingAiClient {
    /// Create a new client from environment configuration.
    pub fn from_env() -> Result<Self, AiError> {
        let config = AiConfig::from_env()?;
        Ok(Self::new(config))
    }

    /// Create a new client with the given configuration.
    pub fn new(config: AiConfig) -> Self {
        // OpenAIClient::builder().build() is fallible in signature only — given
        // a valid api_key and endpoint, the underlying impl never errors.
        let client = OpenAIClient::builder()
            .with_api_key(&config.api_key)
            .with_endpoint(&config.base_url)
            .build()
            .expect("OpenAIClient::build cannot fail with provided api_key and endpoint");

        let cache = AiCache::new(config.cache_dir.clone());

        Self {
            client,
            cache,
            config,
            last_request: Arc::new(Mutex::new(None)),
        }
    }

    /// Apply rate limiting between requests.
    async fn rate_limit(&self) {
        let mut last = self.last_request.lock().await;

        if let Some(last_time) = *last {
            let elapsed = last_time.elapsed();
            let min_interval = Duration::from_millis(self.config.rate_limit_ms);

            if elapsed < min_interval {
                tokio::time::sleep(min_interval - elapsed).await;
            }
        }

        *last = Some(Instant::now());
    }

    /// Convert our ChatMessage to openai-api-rs's format.
    fn to_openai_message(msg: &ChatMessage) -> ChatCompletionMessage {
        let role = match msg.role {
            Role::System => MessageRole::system,
            Role::User => MessageRole::user,
            Role::Assistant => MessageRole::assistant,
        };

        let content = if msg.images.is_empty() {
            Content::Text(msg.content.clone())
        } else {
            // Vision message: heterogeneous content array with text + image parts.
            let mut parts: Vec<ImageUrl> = Vec::with_capacity(1 + msg.images.len());

            parts.push(ImageUrl {
                r#type: ContentType::text,
                text: Some(msg.content.clone()),
                image_url: None,
            });

            for image in &msg.images {
                let data_url = format!("data:{};base64,{}", image.content_type, image.base64);
                parts.push(ImageUrl {
                    r#type: ContentType::image_url,
                    text: None,
                    image_url: Some(ImageUrlType { url: data_url }),
                });
            }

            Content::ImageUrl(parts)
        };

        ChatCompletionMessage {
            role,
            content,
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }
}

#[async_trait]
impl AiClient for CachingAiClient {
    async fn complete(
        &self,
        prompt_name: &str,
        request: ChatRequest,
    ) -> Result<ChatResponse, AiError> {
        // Check cache first
        let cache_key = CacheKey::new(prompt_name, &self.config.model, &request.messages);

        if let Some(cached) = self.cache.get(&cache_key) {
            tracing::debug!(prompt_name = prompt_name, "AI response found in cache");
            return Ok(cached.into());
        }

        // Apply rate limiting
        self.rate_limit().await;

        // Build the request
        let messages: Vec<ChatCompletionMessage> = request
            .messages
            .iter()
            .map(Self::to_openai_message)
            .collect();

        let mut openai_request = ChatCompletionRequest::new(self.config.model.clone(), messages);

        if let Some(max_tokens) = request.max_tokens {
            // openai-api-rs hasn't surfaced max_completion_tokens yet; OpenRouter
            // still accepts the deprecated max_tokens alias.
            openai_request.max_tokens = Some(max_tokens as i64);
        }

        if let Some(temperature) = request.temperature {
            openai_request.temperature = Some(temperature as f64);
        }

        if request.json_response {
            openai_request.response_format = Some(json!({"type": "json_object"}));
        }

        tracing::debug!(
            prompt_name = prompt_name,
            model = &self.config.model,
            "Calling AI API"
        );

        // Make the API call with a hard timeout to avoid hanging the pipeline.
        let response = tokio::time::timeout(
            Duration::from_secs(self.config.request_timeout_secs),
            self.client.chat_completion(openai_request),
        )
        .await
        .map_err(|_| {
            AiError::Api(format!(
                "Request timed out after {}s",
                self.config.request_timeout_secs
            ))
        })?
        .map_err(|e| AiError::Api(e.to_string()))?
        .inner;

        // Extract the response content
        let content = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();

        let usage = Usage {
            prompt_tokens: response.usage.prompt_tokens.max(0) as u32,
            completion_tokens: response.usage.completion_tokens.max(0) as u32,
            total_tokens: response.usage.total_tokens.max(0) as u32,
        };

        let chat_response = ChatResponse {
            content,
            usage,
            cached: false,
        };

        // Cache the response
        if let Err(e) = self
            .cache
            .put(&cache_key, &chat_response, &self.config.model)
        {
            tracing::warn!("Failed to cache AI response: {}", e);
        }

        Ok(chat_response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_path() {
        let key = CacheKey::new(
            "auto_tag",
            "google/gemini-2.5-flash",
            &[ChatMessage::user("test")],
        );

        let path = key.to_path();
        assert!(path.starts_with("auto_tag/google--gemini-2.5-flash/"));
        assert!(path.to_string_lossy().ends_with(".json"));
    }
}
