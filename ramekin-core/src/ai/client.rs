//! AI client implementation using OpenRouter (OpenAI-compatible API).

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
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
    client: Client,
    cache: AiCache,
    config: AiConfig,
    last_request: Arc<Mutex<Option<Instant>>>,
}

#[derive(Debug, Deserialize)]
struct CompletionResponse {
    choices: Vec<CompletionChoice>,
    usage: Option<CompletionUsage>,
}

#[derive(Debug, Deserialize)]
struct CompletionChoice {
    message: CompletionMessage,
}

#[derive(Debug, Deserialize)]
struct CompletionMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CompletionUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

impl CachingAiClient {
    /// Create a new client from environment configuration.
    pub fn from_env() -> Result<Self, AiError> {
        let config = AiConfig::from_env()?;
        Ok(Self::new(config))
    }

    /// Create a new client with the given configuration.
    pub fn new(config: AiConfig) -> Self {
        let client = Client::new();
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

    /// Convert our ChatMessage to an OpenAI-compatible JSON message.
    fn to_api_message(msg: &ChatMessage) -> Value {
        let role = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        };

        match msg.role {
            Role::System | Role::Assistant => json!({
                "role": role,
                "content": msg.content,
            }),
            Role::User => {
                if msg.images.is_empty() {
                    json!({
                        "role": role,
                        "content": msg.content,
                    })
                } else {
                    let mut parts = vec![json!({
                        "type": "text",
                        "text": msg.content,
                    })];

                    for image in &msg.images {
                        let data_url =
                            format!("data:{};base64,{}", image.content_type, image.base64);
                        parts.push(json!({
                            "type": "image_url",
                            "image_url": {
                                "url": data_url,
                                "detail": "high",
                            },
                        }));
                    }

                    json!({
                        "role": role,
                        "content": parts,
                    })
                }
            }
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

        let messages: Vec<Value> = request.messages.iter().map(Self::to_api_message).collect();

        let mut request_body = json!({
            "model": self.config.model,
            "messages": messages,
        });

        let body = request_body
            .as_object_mut()
            .expect("chat completion request must be an object");

        if let Some(max_tokens) = request.max_tokens {
            body.insert("max_completion_tokens".to_string(), json!(max_tokens));
        }

        if let Some(temperature) = request.temperature {
            body.insert("temperature".to_string(), json!(temperature));
        }

        if request.json_response {
            body.insert(
                "response_format".to_string(),
                json!({
                    "type": "json_object",
                }),
            );
        }

        tracing::debug!(
            prompt_name = prompt_name,
            model = &self.config.model,
            "Calling AI API"
        );

        // Make the API call with a hard timeout to avoid hanging the pipeline.
        let url = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );
        let response = tokio::time::timeout(
            Duration::from_secs(self.config.request_timeout_secs),
            self.client
                .post(url)
                .bearer_auth(&self.config.api_key)
                .json(&request_body)
                .send(),
        )
        .await
        .map_err(|_| {
            AiError::Api(format!(
                "Request timed out after {}s",
                self.config.request_timeout_secs
            ))
        })?
        .map_err(|e| AiError::Api(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AiError::Api(format!(
                "Request failed with {}: {}",
                status, body
            )));
        }

        let response: CompletionResponse = response
            .json()
            .await
            .map_err(|e| AiError::ParseError(format!("Failed to parse AI response: {}", e)))?;

        // Extract the response content
        let content = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();

        let usage = response
            .usage
            .map(|u| Usage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            })
            .unwrap_or_default();

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
