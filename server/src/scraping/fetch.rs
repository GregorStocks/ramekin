use crate::db::DbPool;
use crate::models::{NewUrlCache, UrlCache};
use crate::schema::url_cache;
use diesel::prelude::*;
use std::env;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FetchError {
    #[error("URL host not allowed: {0}")]
    HostNotAllowed(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Database error: {0}")]
    DatabaseError(String),
}

/// Check if a URL's host is allowed for scraping.
/// If SCRAPE_ALLOWED_HOSTS is set, only those hosts are allowed.
/// If not set, all hosts are allowed (production mode).
pub fn is_host_allowed(url: &str) -> Result<bool, FetchError> {
    let parsed = reqwest::Url::parse(url).map_err(|e| FetchError::InvalidUrl(e.to_string()))?;

    let host = parsed
        .host_str()
        .ok_or_else(|| FetchError::InvalidUrl("No host in URL".to_string()))?;

    // Check for allowed hosts (used in tests)
    if let Ok(allowed) = env::var("SCRAPE_ALLOWED_HOSTS") {
        let allowed_hosts: Vec<&str> = allowed.split(',').map(|s| s.trim()).collect();
        // Include port if present
        let host_with_port = if let Some(port) = parsed.port() {
            format!("{}:{}", host, port)
        } else {
            host.to_string()
        };

        if !allowed_hosts
            .iter()
            .any(|&h| h == host_with_port || h == host)
        {
            return Err(FetchError::HostNotAllowed(host_with_port));
        }
    }

    Ok(true)
}

/// Get cached content for a URL, if available.
pub fn get_cached(pool: &DbPool, url: &str) -> Result<Option<UrlCache>, FetchError> {
    let mut conn = pool
        .get()
        .map_err(|e| FetchError::DatabaseError(e.to_string()))?;

    url_cache::table
        .find(url)
        .first::<UrlCache>(&mut conn)
        .optional()
        .map_err(|e| FetchError::DatabaseError(e.to_string()))
}

/// Save content to cache.
pub fn save_to_cache(
    pool: &DbPool,
    url: &str,
    content: &[u8],
    content_type: Option<&str>,
) -> Result<(), FetchError> {
    let mut conn = pool
        .get()
        .map_err(|e| FetchError::DatabaseError(e.to_string()))?;

    let new_cache = NewUrlCache {
        url,
        content,
        content_type,
    };

    diesel::insert_into(url_cache::table)
        .values(&new_cache)
        .on_conflict(url_cache::url)
        .do_update()
        .set((
            url_cache::content.eq(content),
            url_cache::content_type.eq(content_type),
            url_cache::fetched_at.eq(diesel::dsl::now),
        ))
        .execute(&mut conn)
        .map_err(|e| FetchError::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Fetch a URL, using cache if available, otherwise fetching from network.
/// Returns the HTML content as a string.
pub async fn fetch_url(pool: &DbPool, url: &str) -> Result<String, FetchError> {
    // Check host allowlist first
    is_host_allowed(url)?;

    // Check cache
    if let Some(cached) = get_cached(pool, url)? {
        tracing::info!("Cache hit for URL: {}", url);
        return String::from_utf8(cached.content).map_err(|e| {
            FetchError::InvalidUrl(format!("Invalid UTF-8 in cached content: {}", e))
        });
    }

    tracing::info!("Cache miss, fetching URL: {}", url);

    // Fetch from network
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (compatible; Ramekin/1.0; +https://ramekin.app)")
        .build()?;

    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(FetchError::RequestFailed(
            response.error_for_status().unwrap_err(),
        ));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let bytes = response.bytes().await?;

    // Save to cache
    save_to_cache(pool, url, &bytes, content_type.as_deref())?;

    String::from_utf8(bytes.to_vec())
        .map_err(|e| FetchError::InvalidUrl(format!("Invalid UTF-8 in response: {}", e)))
}
