//! AI client implementation using OpenRouter (OpenAI-compatible API).

use async_trait::async_trait;
use openai_api_rs::v1::api::OpenAIClient;
use openai_api_rs::v1::chat_completion::chat_completion::ChatCompletionRequest;
use openai_api_rs::v1::chat_completion::{
    ChatCompletionMessage, Content, ContentType, FinishReason, ImageUrl, ImageUrlType, MessageRole,
};
use openai_api_rs::v1::error::APIError;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::time::Instant;

use super::cache::{AiCache, CacheKey};
use super::config::{AiConfig, ConfigError};
use super::types::{ChatMessage, ChatRequest, ChatResponse, Role, Usage};

#[derive(Error, Debug)]
pub enum AiError {
    #[error("API error: {0}")]
    Api(String),

    #[error("Failed to parse response: {0}")]
    ParseError(String),

    /// The answer ran out of `max_tokens`: this request's answer was too long,
    /// so a smaller request (fewer items) may succeed.
    #[error("Response truncated by max_tokens: {0}")]
    Truncated(String),

    #[error("Configuration error: {0}")]
    Config(#[from] super::config::ConfigError),

    /// The provider took longer than the request timeout (image generation,
    /// whose slow models can).
    #[error("Timed out: {0}")]
    Timeout(String),
    /// The provider's content filter refused this request (image
    /// generation): another request may pass.
    #[error("Refused: {0}")]
    Refused(String),
    /// The provider (or the model's upstream host) is rate limiting us: a
    /// transient outage, not a problem with this request.
    #[error("Provider rate limited: {0}")]
    RateLimited(String),
}

impl AiError {
    /// Whether the failure is this request's answer (invalid, cut off by
    /// max_tokens, or refused by a content filter) rather than the provider
    /// or configuration, so asking about
    /// fewer items may succeed where retrying the same request won't.
    pub fn is_answer_specific(&self) -> bool {
        matches!(
            self,
            AiError::ParseError(_) | AiError::Truncated(_) | AiError::Refused(_)
        )
    }
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
        request: &ChatRequest,
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
    request: &ChatRequest,
) -> Result<(T, ChatResponse), AiError> {
    let response = ai_client.complete(prompt_name, request).await?;

    let parse_err = match parse_json::<T>(&response.content) {
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
    let retry = ai_client.complete(prompt_name, request).await?;
    match parse_json::<T>(&retry.content) {
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

/// Whether a model response's content is usable by callers.
///
/// JSON-mode requests need content that parses as JSON: reasoning models can
/// burn the entire `max_tokens` budget on hidden thinking and emit truncated
/// JSON like `{"`, which would otherwise fail every downstream parse.
/// Non-JSON requests just need non-empty content.
fn response_content_usable(json_response: bool, content: &str) -> bool {
    if json_response {
        json_candidates(content)
            .iter()
            .any(|candidate| serde_json::from_str::<serde_json::Value>(candidate).is_ok())
    } else {
        !content.trim().is_empty()
    }
}

/// Where the JSON may be in a JSON-mode response, in order: the whole
/// content, each fenced block (opening fence through its closing fence,
/// whatever text surrounds it, without its "json" info string), and the object
/// the content ends with ("I chose X.\n\n{...}"). Some providers (Anthropic
/// models through OpenRouter) don't honor JSON mode and answer these ways.
/// Callers try each against the type they want (`complete_json`).
fn json_candidates(content: &str) -> Vec<&str> {
    let mut candidates = vec![content.trim()];
    let mut rest = content;
    while let Some(open) = rest.find("```") {
        let Some(after_open) = rest.get(open + 3..) else {
            break;
        };
        let Some(close) = after_open.find("```") else {
            break;
        };
        let (Some(inner), Some(next)) = (after_open.get(..close), after_open.get(close + 3..))
        else {
            break;
        };
        candidates.push(match inner.split_once('\n') {
            Some((info, body)) if !info.contains('{') && !info.contains('[') => body.trim(),
            _ => inner.trim(),
        });
        rest = next;
    }
    let trimmed = content.trim_end();
    if trimmed.ends_with('}') {
        // The earliest '{' from which the rest is valid JSON is the object.
        if let Some(tail) = trimmed
            .match_indices('{')
            .filter_map(|(start, _)| trimmed.get(start..))
            .find(|tail| serde_json::from_str::<serde_json::Value>(tail).is_ok())
        {
            candidates.push(tail);
        }
    }
    candidates
}

/// The first JSON candidate that deserializes as `T`. Otherwise the error of
/// the first candidate that is JSON of the wrong shape (`"servings": 4` where
/// text is wanted), which says what the model got wrong, or else the whole
/// content's.
fn parse_json<T: serde::de::DeserializeOwned>(content: &str) -> Result<T, serde_json::Error> {
    let (mut shape_error, mut first_error) = (None, None);
    for candidate in json_candidates(content) {
        match serde_json::from_str::<T>(candidate) {
            Ok(parsed) => return Ok(parsed),
            Err(e) if e.is_data() => {
                shape_error.get_or_insert(e);
            }
            Err(e) => {
                first_error.get_or_insert(e);
            }
        }
    }
    Err(shape_error
        .or(first_error)
        .expect("there is always the whole-content candidate"))
}

/// The `AiError` for a failed `chat_completion` call.
///
/// OpenRouter can answer HTTP 200 with a choice whose `finish_reason` is
/// `"error"` and which carries an `error` object (e.g. an upstream rate
/// limit). openai-api-rs has no `error` finish reason, so it fails to parse
/// the body and reports `Failed to parse JSON: ... / response {body}`; recover
/// the provider's error from that body rather than surfacing serde's message.
fn chat_completion_error(e: APIError) -> AiError {
    let choice_error = match &e {
        APIError::CustomError { message } if message.starts_with("Failed to parse JSON: ") => {
            message
                .split_once(" / response ")
                .and_then(|(_, body)| serde_json::from_str::<serde_json::Value>(body).ok())
                .and_then(|body| {
                    body["choices"]
                        .as_array()?
                        .iter()
                        .find_map(|choice| choice.get("error").cloned())
                })
        }
        _ => None,
    };
    let Some(error) = choice_error else {
        return AiError::Api(e.to_string());
    };
    let message = error["message"]
        .as_str()
        .unwrap_or("(no message)")
        .to_string();
    if error["code"] == 429 || error["metadata"]["error_type"] == "rate_limit_exceeded" {
        AiError::RateLimited(message)
    } else {
        AiError::Api(format!(
            "Provider error (code {}): {}",
            error["code"], message
        ))
    }
}

/// First ~200 chars of a response, for inclusion in error messages.
fn content_snippet(content: &str) -> String {
    content.chars().take(200).collect()
}

#[async_trait]
impl AiClient for CachingAiClient {
    async fn complete(
        &self,
        prompt_name: &str,
        request: &ChatRequest,
    ) -> Result<ChatResponse, AiError> {
        // Check cache first
        let cache_key = CacheKey::new(prompt_name, &self.config.model, &request.messages);

        if let Some(cached) = (!request.fresh)
            .then(|| self.cache.get(&cache_key))
            .flatten()
        {
            if response_content_usable(request.json_response, &cached.content) {
                tracing::debug!(prompt_name = prompt_name, "AI response found in cache");
                return Ok(cached.into());
            }
            // A poisoned entry (e.g. truncated JSON cached before validation
            // existed) would otherwise fail deterministically forever. Refetch
            // and overwrite it.
            tracing::warn!(
                prompt_name = prompt_name,
                content = %cached.content,
                "Ignoring unusable cached AI response; refetching"
            );
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
        .map_err(chat_completion_error)?
        .inner;

        // Extract the response content
        let choice = response.choices.first();
        let finish_reason = choice.and_then(|c| c.finish_reason.clone());
        let content = choice
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();

        // An unusable response must fail fast and must NOT be cached: caching
        // it would turn a transient provider problem into a permanent,
        // deterministic failure for this prompt.
        if finish_reason == Some(FinishReason::length) {
            return Err(AiError::Truncated(format!(
                "finish_reason=length; partial content: {:?}",
                content_snippet(&content)
            )));
        }

        if !response_content_usable(request.json_response, &content) {
            return Err(AiError::ParseError(format!(
                "Unusable model response (finish_reason={:?}): {:?}",
                finish_reason,
                content_snippet(&content)
            )));
        }

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

/// AI client used when configuration is unavailable (e.g. `OPENROUTER_API_KEY`
/// is unset). Every call fails with the underlying configuration error, so a
/// missing key behaves exactly like an invalid one: AI steps fail when they
/// execute, rather than blocking construction of a pipeline whose non-AI steps
/// could still run.
pub struct UnconfiguredAiClient {
    error: ConfigError,
}

impl UnconfiguredAiClient {
    pub fn new(error: ConfigError) -> Self {
        Self { error }
    }
}

#[async_trait]
impl AiClient for UnconfiguredAiClient {
    async fn complete(
        &self,
        _prompt_name: &str,
        _request: &ChatRequest,
    ) -> Result<ChatResponse, AiError> {
        Err(AiError::Config(self.error.clone()))
    }

    fn forget(&self, _prompt_name: &str, _messages: &[ChatMessage]) {}
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
            _request: &ChatRequest,
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
            fresh: false,
            max_tokens: None,
            temperature: None,
        }
    }

    #[tokio::test]
    async fn complete_json_parses_valid_response() {
        let client = FakeClient::new(vec![response(r#"{"value": "ok"}"#, false)]);

        let (parsed, resp): (TestPayload, _) =
            complete_json(&client, "p", &request()).await.unwrap();

        assert_eq!(parsed.value, "ok");
        assert!(!resp.cached);
        assert!(client.forgotten.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn complete_json_evicts_fresh_unparseable_response() {
        // Truncated JSON, like gemini-2.5-flash stopping after a few tokens.
        let client = FakeClient::new(vec![response(r#"{"value": ""#, false)]);

        let result = complete_json::<TestPayload>(&client, "p", &request()).await;

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
            complete_json(&client, "p", &request()).await.unwrap();

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

        let result = complete_json::<TestPayload>(&client, "p", &request()).await;

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
            image_quality: None,
            ingredient_model: "test-ingredient-model".to_string(),
            extraction_model: "test-extraction-model".to_string(),
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

    #[tokio::test]
    async fn caching_client_fresh_request_skips_the_cached_reply() {
        let dir = tempfile::TempDir::new().unwrap();
        let config = AiConfig {
            api_key: "test-key".to_string(),
            model: "test/model".to_string(),
            image_model: "test/image-model".to_string(),
            image_quality: None,
            ingredient_model: "test-ingredient-model".to_string(),
            extraction_model: "test-extraction-model".to_string(),
            // Nothing listens here, so reaching the provider fails.
            base_url: "http://127.0.0.1:1/v1".to_string(),
            cache_dir: dir.path().to_path_buf(),
            rate_limit_ms: 0,
            request_timeout_secs: 1,
        };
        let request = request();
        let key = CacheKey::new("p", "test/model", &request.messages);
        AiCache::new(config.namespaced_cache_dir())
            .put(
                &key,
                &response(r#"{"value": "cached"}"#, false),
                "test/model",
            )
            .unwrap();
        let client = CachingAiClient::new(config);

        let cached = client.complete("p", &request).await.unwrap();
        assert!(cached.cached);
        let fresh = ChatRequest {
            fresh: true,
            ..request
        };
        assert!(client.complete("p", &fresh).await.is_err());
    }

    #[derive(Debug, PartialEq, serde::Deserialize)]
    struct Answer {
        answer: u32,
    }

    #[test]
    fn json_is_parsed_from_fences_or_after_reasoning() {
        for content in [
            "{\"answer\": 1}",
            "```json\n{\"answer\": 1}\n```",
            "  ```\n{\"answer\": 1}\n```  ",
            "The name means X, so I chose it.\n\n{\"answer\": 1}",
            "Reasoning first.\n```json\n{\"answer\": 1}\n```\n",
            "```json\n{\"answer\": 1}\n```\nThat's my answer.",
            "Example:\n```\nnot json\n```\nAnswer:\n```json\n{\"answer\": 1}\n```\nDone.",
            // Valid JSON of the wrong shape first (an echoed input), then the answer.
            "Input:\n```json\n{\"name\": \"x\"}\n```\nAnswer:\n```json\n{\"answer\": 1}\n```",
        ] {
            assert_eq!(
                parse_json::<Answer>(content).unwrap(),
                Answer { answer: 1 },
                "{content:?}"
            );
            assert!(response_content_usable(true, content), "{content:?}");
        }
        for content in ["No JSON here at all.", "```json\n{\"answer\": \n```"] {
            assert!(parse_json::<Answer>(content).is_err(), "{content:?}");
        }
        // Fenced JSON of the wrong shape reports why, not the fence's syntax.
        let err = parse_json::<Answer>("```json\n{\"answer\": \"one\"}\n```").unwrap_err();
        assert!(err.is_data(), "{err}");
        assert!(!response_content_usable(true, "No JSON here at all."));
    }

    #[test]
    fn invalid_and_truncated_answers_are_answer_specific() {
        // A smaller request may fix these, so batch callers split rather than
        // treating them as a provider outage.
        assert!(AiError::ParseError("bad json".into()).is_answer_specific());
        assert!(AiError::Truncated("finish_reason=length".into()).is_answer_specific());
        assert!(!AiError::Api("Request timed out".into()).is_answer_specific());
        assert!(!AiError::RateLimited("upstream".into()).is_answer_specific());
    }

    /// How openai-api-rs reports a 200 body it can't deserialize.
    fn unparseable_body_error(body: &serde_json::Value) -> APIError {
        APIError::CustomError {
            message: format!(
                "Failed to parse JSON: unknown variant `error`, expected one of `stop`, \
                 `length`, `content_filter`, `tool_calls`, `null` at line 1 column 300 / response {body}"
            ),
        }
    }

    /// A choice-level error body like OpenRouter's prod reply for the
    /// whole-tomato-salad auto-tag on 2026-07-28.
    fn choice_error_body(error: serde_json::Value) -> serde_json::Value {
        json!({
            "id": "gen-1753731234-abc",
            "provider": "Google",
            "model": "google/gemini-2.5-flash",
            "object": "chat.completion",
            "created": 1753731234,
            "choices": [{
                "error": error,
                "logprobs": null,
                "finish_reason": "error",
                "native_finish_reason": "error",
                "index": 0,
                "message": {"role": "assistant", "content": "", "refusal": null, "reasoning": null}
            }],
            "usage": {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}
        })
    }

    #[test]
    fn upstream_rate_limit_in_choice_is_rate_limited() {
        let message = "google/gemini-2.5-flash is temporarily rate-limited upstream. \
                       Please retry shortly, or add your own key to accumulate your rate limits";
        let body = choice_error_body(json!({
            "code": 429,
            "message": message,
            "metadata": {
                "error_type": "rate_limit_exceeded",
                "provider_name": "Google",
                "raw": "google/gemini-2.5-flash is temporarily rate-limited upstream"
            }
        }));

        match chat_completion_error(unparseable_body_error(&body)) {
            AiError::RateLimited(m) => assert_eq!(m, message),
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[test]
    fn other_choice_error_reports_the_provider_message() {
        let body = choice_error_body(json!({"code": 502, "message": "Upstream overloaded"}));

        let err = chat_completion_error(unparseable_body_error(&body));

        assert!(matches!(err, AiError::Api(_)), "{err:?}");
        assert_eq!(
            err.to_string(),
            "API error: Provider error (code 502): Upstream overloaded"
        );
    }

    #[test]
    fn errors_without_a_choice_error_are_unchanged() {
        for e in [
            APIError::CustomError {
                message: "500 Internal Server Error: oops".to_string(),
            },
            APIError::CustomError {
                message: "Failed to parse JSON: EOF / response not json".to_string(),
            },
        ] {
            let expected = format!("API error: {e}");
            let err = chat_completion_error(e);
            assert!(matches!(err, AiError::Api(_)), "{err:?}");
            assert_eq!(err.to_string(), expected);
        }
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

    #[test]
    fn test_response_content_usable_json_mode() {
        assert!(response_content_usable(
            true,
            r#"{"normalized_title": "Mushroom Pasta"}"#
        ));

        // Truncated JSON — what gemini-2.5-flash cached on 2026-05-09 after
        // burning its max_tokens budget on hidden reasoning.
        assert!(!response_content_usable(true, r#"{""#));
        assert!(!response_content_usable(true, r#"{"normalized_title": ""#));
        assert!(!response_content_usable(true, ""));
        // Fenced JSON is usable: some providers ignore JSON mode and fence it.
        assert!(response_content_usable(
            true,
            "```json\n{\"normalized_title\": \"Tuna Boats\"}\n```"
        ));
        // A fence around truncated JSON isn't.
        assert!(!response_content_usable(
            true,
            "```json\n{\"normalized_title\": \n```"
        ));
    }

    #[test]
    fn test_response_content_usable_text_mode() {
        assert!(response_content_usable(false, "plain text answer"));
        assert!(!response_content_usable(false, ""));
        assert!(!response_content_usable(false, "   \n"));
    }

    #[tokio::test]
    async fn unconfigured_client_fails_every_call_with_config_error() {
        let client =
            UnconfiguredAiClient::new(ConfigError::MissingEnvVar("OPENROUTER_API_KEY".to_string()));

        let err = client.complete("p", &request()).await.unwrap_err();

        assert!(
            err.to_string().contains("OPENROUTER_API_KEY"),
            "unexpected error: {err}"
        );

        // forget() has no cache to evict; it must simply be callable.
        client.forget("p", &[ChatMessage::user("hi")]);
    }
}
