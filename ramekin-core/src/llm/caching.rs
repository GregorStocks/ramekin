//! Caching wrapper for LLM providers.
//!
//! Wraps any LlmProvider to cache responses on disk. The cache key is based on
//! the provider name, model name, and prompt hash.

use super::{LlmError, LlmProvider};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

/// Cached LLM response metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetadata {
    pub provider: String,
    pub model: String,
    pub prompt_hash: String,
    pub cached_at: DateTime<Utc>,
}

/// A caching wrapper around any LLM provider.
///
/// Responses are cached to disk based on (provider, model, prompt_hash).
/// Same prompt to same model always returns the same cached response.
#[derive(Debug)]
pub struct CachingProvider {
    inner: Box<dyn LlmProvider>,
    cache_dir: PathBuf,
}

impl CachingProvider {
    /// Create a new CachingProvider wrapping the given provider.
    pub fn new(inner: Box<dyn LlmProvider>, cache_dir: PathBuf) -> Self {
        Self { inner, cache_dir }
    }

    /// Generate a cache key for a prompt.
    ///
    /// Uses SHA-256 for stable hashing across Rust versions.
    fn cache_key(&self, prompt: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(prompt.as_bytes());
        let result = hasher.finalize();

        // Use first 16 bytes (32 hex chars) for shorter filenames
        hex::encode(&result[..16])
    }

    /// Get the cache directory for this provider/model combination.
    fn provider_cache_dir(&self) -> PathBuf {
        self.cache_dir
            .join(self.inner.provider_name())
            .join(self.inner.model_name().replace(['/', ':'], "_"))
    }

    /// Get the path to a cached response.
    fn cache_path(&self, prompt_hash: &str) -> PathBuf {
        self.provider_cache_dir()
            .join(format!("{}.json", prompt_hash))
    }

    /// Try to get a cached response.
    fn get_cached(&self, prompt_hash: &str) -> Option<String> {
        let path = self.cache_path(prompt_hash);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cached) = serde_json::from_str::<CachedLlmResponse>(&content) {
                    tracing::debug!(
                        provider = self.inner.provider_name(),
                        model = self.inner.model_name(),
                        prompt_hash = prompt_hash,
                        "LLM cache hit"
                    );
                    return Some(cached.response);
                }
            }
        }
        None
    }

    /// Save a response to the cache.
    fn save_to_cache(&self, prompt_hash: &str, response: &str) -> Result<(), LlmError> {
        let dir = self.provider_cache_dir();
        fs::create_dir_all(&dir).map_err(|e| LlmError::CacheError(e.to_string()))?;

        let cached = CachedLlmResponse {
            metadata: CacheMetadata {
                provider: self.inner.provider_name().to_string(),
                model: self.inner.model_name().to_string(),
                prompt_hash: prompt_hash.to_string(),
                cached_at: Utc::now(),
            },
            response: response.to_string(),
        };

        let path = self.cache_path(prompt_hash);
        let content = serde_json::to_string_pretty(&cached)
            .map_err(|e| LlmError::CacheError(e.to_string()))?;

        fs::write(&path, content).map_err(|e| LlmError::CacheError(e.to_string()))?;

        tracing::debug!(
            provider = self.inner.provider_name(),
            model = self.inner.model_name(),
            prompt_hash = prompt_hash,
            "LLM response cached"
        );

        Ok(())
    }

    /// Get cache statistics.
    pub fn cache_stats(&self) -> CacheStats {
        let mut stats = CacheStats::default();

        let dir = self.provider_cache_dir();
        if !dir.exists() {
            return stats;
        }

        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                if entry
                    .path()
                    .extension()
                    .map(|e| e == "json")
                    .unwrap_or(false)
                {
                    stats.cached_responses += 1;
                }
            }
        }

        stats
    }
}

/// Cached LLM response with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedLlmResponse {
    metadata: CacheMetadata,
    response: String,
}

/// Cache statistics.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub cached_responses: usize,
}

#[async_trait]
impl LlmProvider for CachingProvider {
    async fn complete(&self, prompt: &str) -> Result<String, LlmError> {
        let prompt_hash = self.cache_key(prompt);

        // Check cache first
        if let Some(cached) = self.get_cached(&prompt_hash) {
            return Ok(cached);
        }

        // Call the underlying provider
        tracing::debug!(
            provider = self.inner.provider_name(),
            model = self.inner.model_name(),
            prompt_hash = %prompt_hash,
            "LLM cache miss, calling provider"
        );

        let response = self.inner.complete(prompt).await?;

        // Save to cache (ignore errors, caching is best-effort)
        if let Err(e) = self.save_to_cache(&prompt_hash, &response) {
            tracing::warn!(error = %e, "Failed to cache LLM response");
        }

        Ok(response)
    }

    fn provider_name(&self) -> &'static str {
        // Return the inner provider's name since we're just a wrapper
        self.inner.provider_name()
    }

    fn model_name(&self) -> &str {
        self.inner.model_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::FakeProvider;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_caching_provider() {
        let temp_dir = TempDir::new().unwrap();
        let fake = Box::new(FakeProvider::with_response("hello", "world"));
        let provider = CachingProvider::new(fake, temp_dir.path().to_path_buf());

        // First call should hit the provider
        let result = provider.complete("hello there").await.unwrap();
        assert_eq!(result, "world");

        // Second call should hit the cache (we can't easily verify this without more instrumentation)
        let result = provider.complete("hello there").await.unwrap();
        assert_eq!(result, "world");

        // Stats should show one cached response
        let stats = provider.cache_stats();
        assert_eq!(stats.cached_responses, 1);
    }

    #[tokio::test]
    async fn test_different_prompts_different_cache() {
        let temp_dir = TempDir::new().unwrap();
        let mut fake = FakeProvider::new();
        fake.add_response("hello", "world");
        fake.add_response("goodbye", "farewell");

        let provider = CachingProvider::new(Box::new(fake), temp_dir.path().to_path_buf());

        provider.complete("hello there").await.unwrap();
        provider.complete("goodbye now").await.unwrap();

        let stats = provider.cache_stats();
        assert_eq!(stats.cached_responses, 2);
    }
}
