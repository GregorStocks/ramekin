//! AI configuration from environment variables.

use std::env;
use thiserror::Error;

/// Default OpenRouter base URL.
pub const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// Default model to use.
pub const DEFAULT_MODEL: &str = "openai/gpt-4o-mini";

/// Default rate limit between requests in milliseconds.
pub const DEFAULT_RATE_LIMIT_MS: u64 = 500;

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
    /// Model name (e.g., "openai/gpt-4o-mini", "anthropic/claude-sonnet-4-20250514").
    pub model: String,
    /// Base URL for the API.
    pub base_url: String,
    /// Directory for caching responses.
    pub cache_dir: std::path::PathBuf,
    /// If true, only use cache, error if not cached.
    pub offline: bool,
    /// Milliseconds to wait between requests.
    pub rate_limit_ms: u64,
}

impl AiConfig {
    /// Load configuration from environment variables.
    ///
    /// Required:
    /// - `OPENROUTER_API_KEY`: API key for OpenRouter
    ///
    /// Optional:
    /// - `RAMEKIN_AI_MODEL`: Model name (default: "openai/gpt-4o-mini")
    /// - `RAMEKIN_AI_BASE_URL`: API base URL (default: "https://openrouter.ai/api/v1")
    /// - `RAMEKIN_AI_CACHE_DIR`: Cache directory (default: "~/.ramekin/ai-cache")
    /// - `RAMEKIN_AI_OFFLINE`: Use cache only (default: false)
    /// - `RAMEKIN_AI_RATE_LIMIT_MS`: Rate limit in ms (default: 500)
    pub fn from_env() -> Result<Self, ConfigError> {
        let api_key = env::var("OPENROUTER_API_KEY")
            .map_err(|_| ConfigError::MissingEnvVar("OPENROUTER_API_KEY".to_string()))?;

        let model = env::var("RAMEKIN_AI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        let base_url =
            env::var("RAMEKIN_AI_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());

        let cache_dir = env::var("RAMEKIN_AI_CACHE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| Self::default_cache_dir());

        let offline = env::var("RAMEKIN_AI_OFFLINE")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let rate_limit_ms = env::var("RAMEKIN_AI_RATE_LIMIT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_RATE_LIMIT_MS);

        Ok(Self {
            api_key,
            model,
            base_url,
            cache_dir,
            offline,
            rate_limit_ms,
        })
    }

    /// Get the default cache directory: ~/.ramekin/ai-cache
    pub fn default_cache_dir() -> std::path::PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".ramekin").join("ai-cache"))
            .unwrap_or_else(|| std::path::PathBuf::from("data/ai-cache"))
    }
}
