//! AI configuration from environment variables.

use std::env;
use thiserror::Error;

/// Default OpenRouter base URL.
pub const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// Default model to use.
pub const DEFAULT_MODEL: &str = "google/gemini-2.5-flash";
/// Default model to use for image generation.
pub const DEFAULT_IMAGE_MODEL: &str = "google/gemini-2.5-flash-image";

/// Default rate limit between requests in milliseconds.
pub const DEFAULT_RATE_LIMIT_MS: u64 = 500;
/// Default request timeout in seconds.
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Missing required environment variable: {0}")]
    MissingEnvVar(String),
}

/// AI client configuration.
#[derive(Debug, Clone)]
pub struct AiConfig {
    /// API key for OpenRouter.
    pub api_key: String,
    /// Model name (e.g., "google/gemini-2.5-flash", "openai/gpt-4.1-mini").
    pub model: String,
    /// Model name for image generation.
    pub image_model: String,
    /// Base URL for the API.
    pub base_url: String,
    /// Directory for caching responses.
    pub cache_dir: std::path::PathBuf,
    /// Milliseconds to wait between requests.
    pub rate_limit_ms: u64,
    /// Seconds before failing an API request.
    pub request_timeout_secs: u64,
}

impl AiConfig {
    /// Load configuration from environment variables.
    ///
    /// Required:
    /// - `OPENROUTER_API_KEY`: API key for OpenRouter
    ///
    /// Optional:
    /// - `RAMEKIN_AI_MODEL`: Model name (default: "google/gemini-2.5-flash")
    /// - `RAMEKIN_AI_IMAGE_MODEL`: Image model name (default: "google/gemini-2.5-flash-image")
    /// - `RAMEKIN_AI_BASE_URL`: API base URL (default: "https://openrouter.ai/api/v1")
    /// - `RAMEKIN_AI_CACHE_DIR`: Cache directory (default: "~/.ramekin/ai-cache")
    /// - `RAMEKIN_AI_RATE_LIMIT_MS`: Rate limit in ms (default: 500)
    /// - `RAMEKIN_AI_TIMEOUT_SECS`: Request timeout in seconds (default: 30)
    pub fn from_env() -> Result<Self, ConfigError> {
        let api_key = env::var("OPENROUTER_API_KEY")
            .map_err(|_| ConfigError::MissingEnvVar("OPENROUTER_API_KEY".to_string()))?;

        let model = env::var("RAMEKIN_AI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        let image_model =
            env::var("RAMEKIN_AI_IMAGE_MODEL").unwrap_or_else(|_| DEFAULT_IMAGE_MODEL.to_string());

        let base_url =
            env::var("RAMEKIN_AI_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());

        let cache_dir = env::var("RAMEKIN_AI_CACHE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| Self::default_cache_dir());

        let rate_limit_ms = env::var("RAMEKIN_AI_RATE_LIMIT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_RATE_LIMIT_MS);

        let request_timeout_secs = env::var("RAMEKIN_AI_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_REQUEST_TIMEOUT_SECS);

        Ok(Self {
            api_key,
            model,
            image_model,
            base_url,
            cache_dir,
            rate_limit_ms,
            request_timeout_secs,
        })
    }

    /// Get the default cache directory: ~/.ramekin/ai-cache
    pub fn default_cache_dir() -> std::path::PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".ramekin").join("ai-cache"))
            .unwrap_or_else(|| std::path::PathBuf::from("data/ai-cache"))
    }
}
