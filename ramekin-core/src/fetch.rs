use crate::error::FetchError;
use std::time::Duration;

/// Build a standard HTTP client with our common configuration.
fn build_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (compatible; Ramekin/1.0; +https://ramekin.app)")
        .build()
}

/// Fetch HTML content from a URL.
///
/// This is a pure HTTP fetch with no caching or host allowlist checking.
/// The caller (server) should perform any policy checks before calling this.
pub async fn fetch_html(url: &str) -> Result<String, FetchError> {
    // Validate URL
    let _parsed = reqwest::Url::parse(url).map_err(|e| FetchError::InvalidUrl(e.to_string()))?;

    let client = build_client()?;

    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(FetchError::RequestFailed(
            response.error_for_status().unwrap_err(),
        ));
    }

    let bytes = response.bytes().await?;

    String::from_utf8(bytes.to_vec())
        .map_err(|e| FetchError::InvalidEncoding(format!("Invalid UTF-8 in response: {}", e)))
}

/// Fetch binary content from a URL (for images, etc.).
///
/// Uses the same HTTP client configuration as fetch_html.
/// The caller (server) should perform any policy checks before calling this.
pub async fn fetch_bytes(url: &str) -> Result<Vec<u8>, FetchError> {
    // Validate URL
    let _parsed = reqwest::Url::parse(url).map_err(|e| FetchError::InvalidUrl(e.to_string()))?;

    let client = build_client()?;

    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(FetchError::RequestFailed(
            response.error_for_status().unwrap_err(),
        ));
    }

    Ok(response.bytes().await?.to_vec())
}
