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

    /// Remove any cached response for this request so the next call re-queries
    /// the provider.
    fn forget(&self, prompt_name: &str, messages: &[ChatMessage]);
}

/// Complete a chat request and parse the response content as JSON into `T`.
///
/// `CachingAiClient` caches responses before callers parse them, so an
/// unparseable response (e.g. a truncated provider flake) would otherwise be
/// pinned in the cache forever. On parse failure this evicts the cache entry
/// so the next run retries the provider; if the bad content came from the
/// cache itself, it retries once with a fresh API call immediately.
pub async fn complete_json<T: serde::de::DeserializeOwned>(
    ai_client: &dyn AiClient,
    prompt_name: &str,
    request: ChatRequest,
) -> Result<(T, ChatResponse), AiError> {
    let response = ai_client.complete(prompt_name, request.clone()).await?;

    let parse_err = match serde_json::from_str::<T>(&response.content) {
        Ok(parsed) => return Ok((parsed, response)),
        Err(e) => e,
    };

    ai_client.forget(prompt_name, &request.messages);

    if !response.cached {
        return Err(parse_error(prompt_name, &response.content, &parse_err));
    }

    // The garbage came from the cache; now that it's evicted, retry fresh.
    tracing::warn!(
        prompt_name = prompt_name,
        "Evicted unparseable cached AI response, retrying: {}",
        parse_err
    );
    let retry = ai_client.complete(prompt_name, request.clone()).await?;
    match serde_json::from_str::<T>(&retry.content) {
        Ok(parsed) => Ok((parsed, retry)),
        Err(e) => {
            ai_client.forget(prompt_name, &request.messages);
            Err(parse_error(prompt_name, &retry.content, &e))
        }
    }
}

fn parse_error(prompt_name: &str, content: &str, err: &serde_json::Error) -> AiError {
    let snippet: String = content.chars().take(200).collect();
    AiError::ParseError(format!(
        "Failed to parse {} response: {}; content: {:?}",
        prompt_name, err, snippet
    ))
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

        let cache = AiCache::new(config.namespaced_cache_dir());

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

    fn forget(&self, prompt_name: &str, messages: &[ChatMessage]) {
        let cache_key = CacheKey::new(prompt_name, &self.config.model, messages);
        if let Err(e) = self.cache.remove(&cache_key) {
            tracing::warn!(
                prompt_name = prompt_name,
                "Failed to evict AI response from cache: {}",
                e
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::config::DEFAULT_BASE_URL;
    use std::collections::VecDeque;
    use std::sync::Mutex as StdMutex;

    /// Scripted AiClient: returns queued responses in order, records forget calls.
    struct FakeClient {
        responses: StdMutex<VecDeque<ChatResponse>>,
        forgotten: StdMutex<Vec<String>>,
    }

    impl FakeClient {
        fn new(responses: Vec<ChatResponse>) -> Self {
            Self {
                responses: StdMutex::new(responses.into()),
                forgotten: StdMutex::new(vec![]),
            }
        }
    }

    #[async_trait]
    impl AiClient for FakeClient {
        async fn complete(
            &self,
            _prompt_name: &str,
            _request: ChatRequest,
        ) -> Result<ChatResponse, AiError> {
            Ok(self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected extra complete() call"))
        }

        fn forget(&self, prompt_name: &str, _messages: &[ChatMessage]) {
            self.forgotten.lock().unwrap().push(prompt_name.to_string());
        }
    }

    #[derive(serde::Deserialize)]
    struct TestPayload {
        value: String,
    }

    fn response(content: &str, cached: bool) -> ChatResponse {
        ChatResponse {
            content: content.to_string(),
            usage: Usage::default(),
            cached,
        }
    }

    fn request() -> ChatRequest {
        ChatRequest {
            messages: vec![ChatMessage::user("hi")],
            json_response: true,
            max_tokens: None,
            temperature: None,
        }
    }

    #[tokio::test]
    async fn complete_json_parses_valid_response() {
        let client = FakeClient::new(vec![response(r#"{"value": "ok"}"#, false)]);

        let (parsed, resp): (TestPayload, _) =
            complete_json(&client, "p", request()).await.unwrap();

        assert_eq!(parsed.value, "ok");
        assert!(!resp.cached);
        assert!(client.forgotten.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn complete_json_evicts_fresh_unparseable_response() {
        // Truncated JSON, like gemini-2.5-flash stopping after a few tokens.
        let client = FakeClient::new(vec![response(r#"{"value": ""#, false)]);

        let result = complete_json::<TestPayload>(&client, "p", request()).await;

        assert!(matches!(result, Err(AiError::ParseError(_))));
        assert_eq!(*client.forgotten.lock().unwrap(), vec!["p"]);
    }

    #[tokio::test]
    async fn complete_json_retries_when_cached_response_is_unparseable() {
        let client = FakeClient::new(vec![
            response(r#"{"value": ""#, true),
            response(r#"{"value": "fresh"}"#, false),
        ]);

        let (parsed, resp): (TestPayload, _) =
            complete_json(&client, "p", request()).await.unwrap();

        assert_eq!(parsed.value, "fresh");
        assert!(!resp.cached);
        assert_eq!(*client.forgotten.lock().unwrap(), vec!["p"]);
    }

    #[tokio::test]
    async fn complete_json_gives_up_when_retry_is_also_unparseable() {
        let client = FakeClient::new(vec![
            response(r#"{"value": ""#, true),
            response("garbage", false),
        ]);

        let result = complete_json::<TestPayload>(&client, "p", request()).await;

        assert!(matches!(result, Err(AiError::ParseError(_))));
        assert_eq!(*client.forgotten.lock().unwrap(), vec!["p", "p"]);
    }

    #[tokio::test]
    async fn caching_client_forget_removes_cache_entry() {
        let dir = tempfile::TempDir::new().unwrap();
        let config = AiConfig {
            api_key: "test-key".to_string(),
            model: "test/model".to_string(),
            image_model: "test/image-model".to_string(),
            base_url: DEFAULT_BASE_URL.to_string(),
            cache_dir: dir.path().to_path_buf(),
            rate_limit_ms: 0,
            request_timeout_secs: 1,
        };
        let messages = vec![ChatMessage::user("hi")];
        let key = CacheKey::new("p", "test/model", &messages);
        let cache = AiCache::new(config.namespaced_cache_dir());
        cache
            .put(&key, &response("{}", false), "test/model")
            .unwrap();
        assert!(cache.get(&key).is_some());

        let client = CachingAiClient::new(config);
        client.forget("p", &messages);

        assert!(cache.get(&key).is_none());
    }

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
