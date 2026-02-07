//! Unified HTTP client with caching and rate limiting.
//!
//! All outgoing HTTP requests should go through this module to ensure
//! consistent caching behavior and avoid hammering external servers.

mod cache;
pub(crate) mod charset;
mod client;
mod rate_limiter;

pub use cache::{CacheStats, CachedError, CachedResponse, DiskCache};
pub use client::{CachingClient, CachingClientBuilder, HttpClient, MockClient, MockResponse};
pub use rate_limiter::RateLimiter;

/// Convert a URL to a filesystem-safe slug.
/// e.g., "https://www.seriouseats.com/best-chili-recipe-123" -> "seriouseats-com_best-chili-recipe-123"
pub fn slugify_url(url: &str) -> String {
    let parsed = match url::Url::parse(url) {
        Ok(p) => p,
        Err(_) => return sanitize_for_filesystem(url),
    };

    let host = parsed
        .host_str()
        .unwrap_or("unknown")
        .trim_start_matches("www.");

    let path = parsed.path().trim_matches('/');

    // Combine host and path, replacing special chars
    let combined = if path.is_empty() {
        host.to_string()
    } else {
        format!("{}_{}", host, path)
    };

    sanitize_for_filesystem(&combined)
}

fn sanitize_for_filesystem(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c == '.' || c == '/' {
                '-'
            } else {
                '_'
            }
        })
        .collect::<String>()
        .chars()
        .take(200) // Limit length for filesystem compatibility
        .collect()
}
