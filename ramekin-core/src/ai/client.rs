//! AI client implementation using OpenRouter (OpenAI-compatible API).

use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs, ResponseFormat,
    },
    Client,
};
use async_trait::async_trait;
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

    #[error("Response not in cache and offline mode is enabled")]
    OfflineNotCached,

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
    client: Client<OpenAIConfig>,
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
        // Configure async-openai to use OpenRouter
        let openai_config = OpenAIConfig::new()
            .with_api_key(&config.api_key)
            .with_api_base(&config.base_url);

        let client = Client::with_config(openai_config);
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

    /// Convert our ChatMessage to async-openai's format.
    fn to_openai_message(msg: &ChatMessage) -> Result<ChatCompletionRequestMessage, AiError> {
        match msg.role {
            Role::System => ChatCompletionRequestSystemMessageArgs::default()
                .content(msg.content.clone())
                .build()
                .map(Into::into)
                .map_err(|e| AiError::Api(format!("Failed to build system message: {}", e))),
            Role::User => ChatCompletionRequestUserMessageArgs::default()
                .content(msg.content.clone())
                .build()
                .map(Into::into)
                .map_err(|e| AiError::Api(format!("Failed to build user message: {}", e))),
            Role::Assistant => {
                use async_openai::types::chat::ChatCompletionRequestAssistantMessageArgs;
                ChatCompletionRequestAssistantMessageArgs::default()
                    .content(msg.content.clone())
                    .build()
                    .map(Into::into)
                    .map_err(|e| AiError::Api(format!("Failed to build assistant message: {}", e)))
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

        // If offline mode, error
        if self.config.offline {
            return Err(AiError::OfflineNotCached);
        }

        // Apply rate limiting
        self.rate_limit().await;

        // Build the request
        let messages: Vec<ChatCompletionRequestMessage> = request
            .messages
            .iter()
            .map(Self::to_openai_message)
            .collect::<Result<Vec<_>, _>>()?;

        let mut req_builder = CreateChatCompletionRequestArgs::default();
        req_builder.model(&self.config.model).messages(messages);

        if let Some(max_tokens) = request.max_tokens {
            req_builder.max_completion_tokens(max_tokens);
        }

        if let Some(temperature) = request.temperature {
            req_builder.temperature(temperature);
        }

        if request.json_response {
            req_builder.response_format(ResponseFormat::JsonObject);
        }

        let openai_request = req_builder
            .build()
            .map_err(|e| AiError::Api(e.to_string()))?;

        tracing::debug!(
            prompt_name = prompt_name,
            model = &self.config.model,
            "Calling AI API"
        );

        // Make the API call
        let response = self
            .client
            .chat()
            .create(openai_request)
            .await
            .map_err(|e| AiError::Api(e.to_string()))?;

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
            "openai/gpt-4o-mini",
            &[ChatMessage::user("test")],
        );

        let path = key.to_path();
        assert!(path.starts_with("auto_tag/openai--gpt-4o-mini/"));
        assert!(path.to_string_lossy().ends_with(".json"));
    }
}
