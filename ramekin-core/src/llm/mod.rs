//! LLM provider abstraction for recipe enrichment.
//!
//! This module provides a trait-based abstraction over different LLM providers
//! (Claude, OpenAI, etc.) with support for caching and testing.

mod caching;
mod claude;
mod fake;

pub use caching::CachingProvider;
pub use claude::ClaudeProvider;
pub use fake::FakeProvider;

use async_trait::async_trait;
use std::fmt;
use thiserror::Error;

/// Error type for LLM operations.
#[derive(Debug, Error)]
pub enum LlmError {
    #[error("API request failed: {0}")]
    RequestFailed(String),

    #[error("API returned error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Failed to parse response: {0}")]
    ParseError(String),

    #[error("Rate limited, retry after {retry_after_secs:?} seconds")]
    RateLimited { retry_after_secs: Option<u64> },

    #[error("Provider not configured: {0}")]
    NotConfigured(String),

    #[error("Cache error: {0}")]
    CacheError(String),
}

/// Trait for LLM providers.
///
/// Implementations should be stateless and thread-safe. The provider is responsible
/// for making API calls and returning the model's text response.
#[async_trait]
pub trait LlmProvider: Send + Sync + fmt::Debug {
    /// Send a prompt to the LLM and get a text response.
    async fn complete(&self, prompt: &str) -> Result<String, LlmError>;

    /// Get the provider name (e.g., "claude", "openai", "fake").
    fn provider_name(&self) -> &'static str;

    /// Get the model name (e.g., "claude-3-5-sonnet-20241022").
    fn model_name(&self) -> &str;
}

/// Registry of available providers.
///
/// Use environment variables to configure:
/// - ENRICHMENT_PROVIDER: "claude" | "openai" | "fake"
/// - ENRICHMENT_MODEL: Model name (provider-specific)
/// - ANTHROPIC_API_KEY: API key for Claude
/// - OPENAI_API_KEY: API key for OpenAI
pub fn create_provider_from_env() -> Result<Box<dyn LlmProvider>, LlmError> {
    let provider = std::env::var("ENRICHMENT_PROVIDER").unwrap_or_else(|_| "fake".to_string());

    match provider.as_str() {
        "fake" => Ok(Box::new(FakeProvider::default())),
        "claude" => {
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| LlmError::NotConfigured("ANTHROPIC_API_KEY not set".to_string()))?;
            let model = std::env::var("ENRICHMENT_MODEL")
                .unwrap_or_else(|_| "claude-3-5-sonnet-20241022".to_string());
            Ok(Box::new(ClaudeProvider::new(api_key, model)))
        }
        "openai" => Err(LlmError::NotConfigured(
            "OpenAI provider not yet implemented".to_string(),
        )),
        other => Err(LlmError::NotConfigured(format!(
            "Unknown provider: {}",
            other
        ))),
    }
}

/// Create a provider with caching enabled.
///
/// Cache directory is determined by ENRICHMENT_CACHE_DIR or defaults to .cache/enrichment.
pub fn create_cached_provider_from_env() -> Result<Box<dyn LlmProvider>, LlmError> {
    let inner = create_provider_from_env()?;

    let cache_dir = std::env::var("ENRICHMENT_CACHE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(".cache/enrichment"));

    Ok(Box::new(CachingProvider::new(inner, cache_dir)))
}
