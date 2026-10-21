//! Disk-based HTTP response cache with ETag support.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use super::slugify_url;
use crate::cache_io::{parse_or_warn, read_or_warn};

/// Disk-based HTTP response cache.
pub struct DiskCache {
    cache_dir: PathBuf,
}

/// Metadata stored alongside cached responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetadata {
    pub url: String,
    pub content_type: Option<String>,
    pub fetched_at: DateTime<Utc>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

/// A cached successful response.
#[derive(Debug, Clone)]
pub struct CachedResponse {
    pub data: Vec<u8>,
    pub metadata: CacheMetadata,
}

/// A cached error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedError {
    pub error: String,
    pub fetched_at: DateTime<Utc>,
}

/// Cache statistics.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub cached_success: usize,
    pub cached_errors: usize,
}

impl DiskCache {
    /// Create a new DiskCache with the given directory.
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Get the default cache directory: ~/.ramekin/http-cache
    pub fn default_dir() -> PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".ramekin").join("http-cache"))
            .unwrap_or_else(|| PathBuf::from("data/http-cache"))
    }

    /// Get the directory for a specific URL's cache.
    fn url_dir(&self, url: &str) -> PathBuf {
        self.cache_dir.join(slugify_url(url))
    }

    /// Check if a response is cached for a URL (either success or error).
    pub fn is_cached(&self, url: &str) -> bool {
        let dir = self.url_dir(url);
        dir.join("response.bin").exists() || self.get_error(url).is_some()
    }

    /// Get cached response if it exists.
    ///
    /// Corrupt or unreadable entries are treated as cache misses (the caller
    /// falls back to the network), but loudly: silent misses would hide cache
    /// corruption behind re-fetches.
    pub fn get(&self, url: &str) -> Option<CachedResponse> {
        let dir = self.url_dir(url);
        let response_path = dir.join("response.bin");
        let metadata_path = dir.join("metadata.json");

        if response_path.exists() && metadata_path.exists() {
            let data = read_or_warn(&response_path, fs::read(&response_path))?;
            let metadata_str = read_or_warn(&metadata_path, fs::read_to_string(&metadata_path))?;
            let metadata: CacheMetadata = parse_or_warn(&metadata_path, &metadata_str)?;
            Some(CachedResponse { data, metadata })
        } else {
            None
        }
    }

    /// Get cached error if it exists.
    ///
    /// Only permanent errors (404/410) count. Older caches also stored 403s
    /// and 429s, which depend on who's asking (e.g. a User-Agent block) and
    /// would otherwise never be retried, so those are treated as misses.
    pub fn get_error(&self, url: &str) -> Option<CachedError> {
        let dir = self.url_dir(url);
        let error_path = dir.join("error.txt");

        if error_path.exists() {
            let error_str = read_or_warn(&error_path, fs::read_to_string(&error_path))?;
            let cached: CachedError = parse_or_warn(&error_path, &error_str)?;
            is_permanent_error(&cached.error).then_some(cached)
        } else {
            None
        }
    }

    /// Get cache metadata without loading the full response.
    pub fn get_metadata(&self, url: &str) -> Option<CacheMetadata> {
        let dir = self.url_dir(url);
        let metadata_path = dir.join("metadata.json");

        if metadata_path.exists() {
            let metadata_str = read_or_warn(&metadata_path, fs::read_to_string(&metadata_path))?;
            parse_or_warn(&metadata_path, &metadata_str)
        } else {
            None
        }
    }

    /// Save a successful response to the cache.
    pub fn put(
        &self,
        url: &str,
        data: &[u8],
        content_type: Option<String>,
        etag: Option<String>,
        last_modified: Option<String>,
    ) -> std::io::Result<()> {
        let dir = self.url_dir(url);
        fs::create_dir_all(&dir)?;

        let metadata = CacheMetadata {
            url: url.to_string(),
            content_type,
            fetched_at: Utc::now(),
            etag,
            last_modified,
        };

        fs::write(dir.join("response.bin"), data)?;
        fs::write(
            dir.join("metadata.json"),
            serde_json::to_string_pretty(&metadata).unwrap(),
        )?;

        // Remove any cached error if we succeeded
        let error_path = dir.join("error.txt");
        if error_path.exists() {
            let _ = fs::remove_file(error_path);
        }

        Ok(())
    }

    /// Save an error to the cache (negative caching).
    pub fn put_error(&self, url: &str, error: &str) -> std::io::Result<()> {
        let dir = self.url_dir(url);
        fs::create_dir_all(&dir)?;

        let cached_error = CachedError {
            error: error.to_string(),
            fetched_at: Utc::now(),
        };

        fs::write(
            dir.join("error.txt"),
            serde_json::to_string_pretty(&cached_error).unwrap(),
        )?;

        Ok(())
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        let mut stats = CacheStats::default();

        if !self.cache_dir.exists() {
            return stats;
        }

        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                if entry.path().is_dir() {
                    if entry.path().join("response.bin").exists() {
                        stats.cached_success += 1;
                    } else if entry.path().join("error.txt").exists() {
                        stats.cached_errors += 1;
                    }
                }
            }
        }

        stats
    }

    /// Clear all cached responses.
    pub fn clear(&self) -> std::io::Result<()> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir)?;
        }
        Ok(())
    }
}

/// HTTP statuses that mean the URL itself is gone, so caching the failure is safe.
pub fn is_permanent_status(status: reqwest::StatusCode) -> bool {
    matches!(
        status,
        reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE
    )
}

/// Cached errors are stored as `format!("HTTP {status}")`.
fn is_permanent_error(error: &str) -> bool {
    error.starts_with("HTTP 404 ") || error.starts_with("HTTP 410 ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://example.com/recipe";

    #[test]
    fn corrupt_metadata_is_a_cache_miss() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf());
        cache.put(URL, b"body", None, None, None).unwrap();
        assert!(cache.get(URL).is_some());

        fs::write(
            dir.path().join(slugify_url(URL)).join("metadata.json"),
            "not json",
        )
        .unwrap();

        assert!(cache.get(URL).is_none());
        assert!(cache.get_metadata(URL).is_none());
    }

    #[test]
    fn corrupt_cached_error_is_a_cache_miss() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf());
        cache.put_error(URL, "HTTP 404 Not Found").unwrap();
        assert!(cache.get_error(URL).is_some());

        fs::write(
            dir.path().join(slugify_url(URL)).join("error.txt"),
            "not json",
        )
        .unwrap();

        assert!(cache.get_error(URL).is_none());
    }

    #[test]
    fn non_permanent_cached_error_is_a_cache_miss() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf());

        cache.put_error(URL, "HTTP 403 Forbidden").unwrap();
        assert!(cache.get_error(URL).is_none());
        assert!(!cache.is_cached(URL));

        cache.put_error(URL, "HTTP 410 Gone").unwrap();
        assert!(cache.get_error(URL).is_some());
        assert!(cache.is_cached(URL));
    }
}
