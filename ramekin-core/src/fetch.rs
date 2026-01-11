//! Convenience functions for HTTP fetching.
//!
//! These are thin wrappers around CachingClient for backwards compatibility.
//! For more control, use CachingClient directly.

use crate::error::FetchError;
use crate::http::{CachingClient, HttpClient};

/// Fetch HTML content from a URL.
///
/// Uses the default CachingClient configuration (disk caching, rate limiting).
/// For more control over caching behavior, use CachingClient directly.
pub async fn fetch_html(url: &str) -> Result<String, FetchError> {
    let client = CachingClient::new()?;
    client.fetch_html(url).await
}

/// Fetch binary content from a URL (for images, etc.).
///
/// Uses the default CachingClient configuration (disk caching, rate limiting).
/// For more control over caching behavior, use CachingClient directly.
pub async fn fetch_bytes(url: &str) -> Result<Vec<u8>, FetchError> {
    let client = CachingClient::new()?;
    client.fetch_bytes(url).await
}
