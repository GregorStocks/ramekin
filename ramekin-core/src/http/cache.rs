//! Disk-based HTTP response cache with ETag support.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use super::slugify_url;

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
        dir.join("response.bin").exists() || dir.join("error.txt").exists()
    }

    /// Get cached response if it exists.
    pub fn get(&self, url: &str) -> Option<CachedResponse> {
        let dir = self.url_dir(url);
        let response_path = dir.join("response.bin");
        let metadata_path = dir.join("metadata.json");

        if response_path.exists() && metadata_path.exists() {
            let data = fs::read(&response_path).ok()?;
            let metadata_str = fs::read_to_string(&metadata_path).ok()?;
            let metadata: CacheMetadata = serde_json::from_str(&metadata_str).ok()?;
            Some(CachedResponse { data, metadata })
        } else {
            None
        }
    }

    /// Get cached error if it exists.
    pub fn get_error(&self, url: &str) -> Option<CachedError> {
        let dir = self.url_dir(url);
        let error_path = dir.join("error.txt");

        if error_path.exists() {
            let error_str = fs::read_to_string(&error_path).ok()?;
            serde_json::from_str(&error_str).ok()
        } else {
            None
        }
    }

    /// Get cache metadata without loading the full response.
    pub fn get_metadata(&self, url: &str) -> Option<CacheMetadata> {
        let dir = self.url_dir(url);
        let metadata_path = dir.join("metadata.json");

        if metadata_path.exists() {
            let metadata_str = fs::read_to_string(&metadata_path).ok()?;
            serde_json::from_str(&metadata_str).ok()
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
