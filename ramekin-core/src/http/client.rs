//! HTTP client trait and implementations.

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::error::FetchError;

use super::cache::DiskCache;
use super::charset;
use super::rate_limiter::RateLimiter;

/// Result of a fetch operation, including content-type for charset detection.
struct FetchResult {
    data: Vec<u8>,
    content_type: Option<String>,
}

/// Trait for HTTP clients, enabling mockability in tests.
#[async_trait]
pub trait HttpClient: Send + Sync {
    /// Fetch HTML content from a URL.
    async fn fetch_html(&self, url: &str) -> Result<String, FetchError>;

    /// Fetch binary content from a URL.
    async fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, FetchError>;
}

/// Configuration for CachingClient.
#[derive(Clone)]
pub struct CachingClientBuilder {
    cache_dir: Option<PathBuf>,
    rate_limit_ms: u64,
    never_network: bool,
    timeout: Duration,
    user_agent: String,
}

impl Default for CachingClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CachingClientBuilder {
    /// Create a new builder with default settings.
    ///
    /// Environment variables:
    /// - `RAMEKIN_HTTP_CACHE`: "none" to disable, "disk" (default), or a path
    /// - `RAMEKIN_OFFLINE`: "true" to never hit network (error if not cached)
    ///
    /// Cache hits are always served without revalidation. Use `FORCE_REFETCH=1`
    /// or `make pipeline-cache-clear` to pick up upstream changes.
    pub fn new() -> Self {
        // Check environment variables for configuration
        let cache_dir = match std::env::var("RAMEKIN_HTTP_CACHE").ok() {
            Some(val) if val == "none" => None,
            Some(val) if val == "disk" => Some(DiskCache::default_dir()),
            Some(path) => Some(PathBuf::from(path)),
            None => Some(DiskCache::default_dir()), // Default to disk caching
        };

        let never_network = std::env::var("RAMEKIN_OFFLINE")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        Self {
            cache_dir,
            rate_limit_ms: 200, // Default 200ms between requests to same host
            never_network,
            timeout: Duration::from_secs(30),
            user_agent: "Mozilla/5.0 (compatible; Ramekin/1.0; +https://ramekin.app)".to_string(),
        }
    }

    /// Set the cache directory. None disables caching.
    pub fn cache_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.cache_dir = dir;
        self
    }

    /// Set the rate limit delay in milliseconds. 0 disables rate limiting.
    pub fn rate_limit_ms(mut self, ms: u64) -> Self {
        self.rate_limit_ms = ms;
        self
    }

    /// Set never-network mode. When true, returns an error if content is not cached.
    pub fn never_network(mut self, never: bool) -> Self {
        self.never_network = never;
        self
    }

    /// Set the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the user agent string.
    pub fn user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = user_agent;
        self
    }

    /// Build the CachingClient.
    pub fn build(self) -> Result<CachingClient, reqwest::Error> {
        let inner = reqwest::Client::builder()
            .timeout(self.timeout)
            .user_agent(&self.user_agent)
            .build()?;

        let cache = self.cache_dir.map(DiskCache::new);
        let rate_limiter = RateLimiter::new(Duration::from_millis(self.rate_limit_ms));

        Ok(CachingClient {
            inner: Arc::new(inner),
            cache,
            rate_limiter,
            never_network: self.never_network,
        })
    }
}

/// Production HTTP client with a disk cache and per-host rate limiting.
///
/// Cache hits are served directly without revalidating against the origin;
/// we never issue an `If-None-Match` / `If-Modified-Since` round-trip for a
/// URL that has a cached response. Clearing or bypassing the cache is the
/// only way to pick up upstream changes.
pub struct CachingClient {
    /// Shared reqwest client for connection pooling.
    inner: Arc<reqwest::Client>,
    /// Optional disk cache.
    cache: Option<DiskCache>,
    /// Per-host rate limiter.
    rate_limiter: RateLimiter,
    /// When true, never access network - return error if not cached.
    never_network: bool,
}

impl CachingClient {
    /// Create a new CachingClient with default configuration.
    pub fn new() -> Result<Self, reqwest::Error> {
        CachingClientBuilder::new().build()
    }

    /// Get a builder for custom configuration.
    pub fn builder() -> CachingClientBuilder {
        CachingClientBuilder::new()
    }

    /// Get cache statistics.
    pub fn cache_stats(&self) -> Option<super::cache::CacheStats> {
        self.cache.as_ref().map(|c| c.stats())
    }

    /// Clear the cache.
    pub fn clear_cache(&self) -> std::io::Result<()> {
        if let Some(cache) = &self.cache {
            cache.clear()?;
        }
        Ok(())
    }

    /// Invalidate the cache entry for a single URL so the next fetch goes to
    /// the network. Used by `FORCE_REFETCH` to refresh a specific URL without
    /// clearing the entire cache.
    pub fn invalidate_cache(&self, url: &str) -> std::io::Result<()> {
        if let Some(cache) = &self.cache {
            cache.remove(url)?;
        }
        Ok(())
    }

    /// Check if a URL is cached.
    pub fn is_cached(&self, url: &str) -> bool {
        self.cache
            .as_ref()
            .map(|c| c.is_cached(url))
            .unwrap_or(false)
    }

    /// Get cached HTML if it exists (without fetching).
    pub fn get_cached_html(&self, url: &str) -> Option<String> {
        self.cache.as_ref().and_then(|c| {
            c.get(url).map(|resp| {
                charset::decode_bytes_to_utf8(resp.data, resp.metadata.content_type.as_deref())
            })
        })
    }

    /// Get cached bytes if they exist (without fetching).
    pub fn get_cached_bytes(&self, url: &str) -> Option<Vec<u8>> {
        self.cache.as_ref().and_then(|c| c.get(url).map(|r| r.data))
    }

    /// Get cached error if it exists.
    pub fn get_cached_error(&self, url: &str) -> Option<String> {
        self.cache
            .as_ref()
            .and_then(|c| c.get_error(url).map(|e| e.error))
    }

    /// Inject HTML content into the cache without fetching.
    /// Useful for manually saving pages.
    pub fn inject_html(&self, url: &str, html: &str) -> std::io::Result<()> {
        if let Some(cache) = &self.cache {
            cache.put(
                url,
                html.as_bytes(),
                Some("text/html".to_string()),
                None,
                None,
            )?;
        }
        Ok(())
    }

    /// Inject bytes content into the cache without fetching.
    pub fn inject_bytes(
        &self,
        url: &str,
        data: &[u8],
        content_type: Option<String>,
    ) -> std::io::Result<()> {
        if let Some(cache) = &self.cache {
            cache.put(url, data, content_type, None, None)?;
        }
        Ok(())
    }

    /// Extract host from URL for rate limiting.
    fn get_host(url: &str) -> Option<String> {
        reqwest::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
    }

    /// Internal fetch implementation with caching logic.
    ///
    /// Cache hits are served directly. We do not revalidate against the origin
    /// — see the `CachingClient` doc comment for the rationale.
    async fn fetch_with_cache(&self, url: &str) -> Result<FetchResult, FetchError> {
        // Validate URL first
        let parsed = reqwest::Url::parse(url).map_err(|e| FetchError::InvalidUrl(e.to_string()))?;

        // Check cache
        if let Some(cache) = &self.cache {
            // Check for cached error first
            if let Some(cached_error) = cache.get_error(url) {
                tracing::debug!(url, error = %cached_error.error, "cache hit (cached error)");
                return Err(FetchError::InvalidUrl(format!(
                    "Cached error: {}",
                    cached_error.error
                )));
            }

            // Cache hit: serve the cached response without revalidating.
            if let Some(cached) = cache.get(url) {
                tracing::debug!(url, "cache hit");
                return Ok(FetchResult {
                    data: cached.data,
                    content_type: cached.metadata.content_type,
                });
            }
        }

        // No cache or no cached response
        if self.never_network {
            tracing::debug!(url, "cache miss (RAMEKIN_OFFLINE set, network disabled)");
            return Err(FetchError::InvalidUrl(format!(
                "URL not cached and RAMEKIN_OFFLINE is set: {}",
                url
            )));
        }

        // Fetch from network
        if let Some(host) = Self::get_host(url) {
            self.rate_limiter.wait(&host).await;
        }

        tracing::debug!(url, "network: fetching (not cached)");
        let response = self.inner.get(parsed).send().await?;

        if !response.status().is_success() {
            let error_msg = format!("HTTP {}", response.status());
            tracing::debug!(url, status = %response.status(), "network: request failed");
            if let Some(cache) = &self.cache {
                let _ = cache.put_error(url, &error_msg);
            }
            return Err(FetchError::RequestFailed(
                response.error_for_status().unwrap_err(),
            ));
        }

        tracing::debug!(url, status = %response.status(), "network: fetched successfully");
        let etag = response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let last_modified = response
            .headers()
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let bytes = response.bytes().await?.to_vec();

        // Save to cache
        if let Some(cache) = &self.cache {
            let _ = cache.put(url, &bytes, content_type.clone(), etag, last_modified);
        }

        Ok(FetchResult {
            data: bytes,
            content_type,
        })
    }
}

#[async_trait]
impl HttpClient for CachingClient {
    async fn fetch_html(&self, url: &str) -> Result<String, FetchError> {
        let result = self.fetch_with_cache(url).await?;
        Ok(charset::decode_bytes_to_utf8(
            result.data,
            result.content_type.as_deref(),
        ))
    }

    async fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        Ok(self.fetch_with_cache(url).await?.data)
    }
}

/// Allow sharing a CachingClient via Arc for use in the generic pipeline.
#[async_trait]
impl HttpClient for Arc<CachingClient> {
    async fn fetch_html(&self, url: &str) -> Result<String, FetchError> {
        (**self).fetch_html(url).await
    }

    async fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        (**self).fetch_bytes(url).await
    }
}

/// Mock response for testing.
#[derive(Clone)]
pub enum MockResponse {
    Html(String),
    Bytes(Vec<u8>),
    Error(String),
}

/// Mock HTTP client for testing.
pub struct MockClient {
    responses: HashMap<String, MockResponse>,
}

impl MockClient {
    /// Create a new empty mock client.
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
        }
    }

    /// Add a response for a URL.
    pub fn with_response(mut self, url: &str, response: MockResponse) -> Self {
        self.responses.insert(url.to_string(), response);
        self
    }

    /// Add an HTML response for a URL.
    pub fn with_html(self, url: &str, html: &str) -> Self {
        self.with_response(url, MockResponse::Html(html.to_string()))
    }

    /// Add a bytes response for a URL.
    pub fn with_bytes(self, url: &str, bytes: Vec<u8>) -> Self {
        self.with_response(url, MockResponse::Bytes(bytes))
    }

    /// Add an error response for a URL.
    pub fn with_error(self, url: &str, error: &str) -> Self {
        self.with_response(url, MockResponse::Error(error.to_string()))
    }
}

impl Default for MockClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpClient for MockClient {
    async fn fetch_html(&self, url: &str) -> Result<String, FetchError> {
        match self.responses.get(url) {
            Some(MockResponse::Html(html)) => Ok(html.clone()),
            Some(MockResponse::Bytes(bytes)) => {
                Ok(charset::decode_bytes_to_utf8(bytes.clone(), None))
            }
            Some(MockResponse::Error(e)) => Err(FetchError::InvalidUrl(e.clone())),
            None => Err(FetchError::InvalidUrl(format!(
                "No mock response for URL: {}",
                url
            ))),
        }
    }

    async fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        match self.responses.get(url) {
            Some(MockResponse::Html(html)) => Ok(html.as_bytes().to_vec()),
            Some(MockResponse::Bytes(bytes)) => Ok(bytes.clone()),
            Some(MockResponse::Error(e)) => Err(FetchError::InvalidUrl(e.clone())),
            None => Err(FetchError::InvalidUrl(format!(
                "No mock response for URL: {}",
                url
            ))),
        }
    }
}
