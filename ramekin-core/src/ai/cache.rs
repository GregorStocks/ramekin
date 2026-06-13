//! Disk-based AI response cache.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

use super::types::{ChatMessage, ChatResponse, Usage};
use crate::cache_io::{parse_or_warn, read_or_warn};

/// Disk-based AI response cache.
pub struct AiCache {
    cache_dir: PathBuf,
}

/// Metadata for a cached response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedAiResponse {
    pub content: String,
    pub usage: Usage,
    pub cached_at: DateTime<Utc>,
    pub model: String,
}

impl From<CachedAiResponse> for ChatResponse {
    fn from(cached: CachedAiResponse) -> Self {
        Self {
            content: cached.content,
            usage: cached.usage,
            cached: true,
        }
    }
}

/// Cache key components.
#[derive(Debug, Clone)]
pub struct CacheKey {
    pub prompt_name: String,
    pub model: String,
    pub input_hash: String,
}

impl CacheKey {
    /// Create a new cache key from the given components.
    ///
    /// The cache key is based on the prompt name, model, and a hash of the messages.
    /// Since the messages include the full rendered prompt, any changes to the prompt
    /// template will automatically invalidate the cache (no manual version bumping needed).
    pub fn new(prompt_name: &str, model: &str, messages: &[ChatMessage]) -> Self {
        let input_json = serde_json::to_string(messages).expect("chat messages serialize to JSON");
        let input_hash = sha256_hex(&input_json);

        Self {
            prompt_name: prompt_name.to_string(),
            model: model.to_string(),
            input_hash,
        }
    }

    /// Convert to a filesystem path relative to the cache directory.
    ///
    /// Format: {prompt_name}/{model_safe}/{hash[0:2]}/{hash}.json
    pub fn to_path(&self) -> PathBuf {
        // Replace slashes in model name (e.g., "google/gemini-2.5-flash" -> "google--gemini-2.5-flash")
        let model_safe = self.model.replace('/', "--");
        let hash_prefix: String = self.input_hash.chars().take(2).collect();

        PathBuf::new()
            .join(&self.prompt_name)
            .join(&model_safe)
            .join(&hash_prefix)
            .join(format!("{}.json", &self.input_hash))
    }
}

impl AiCache {
    /// Create a new cache with the given directory.
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Get a cached response if it exists.
    ///
    /// A corrupt or unreadable entry is treated as a cache miss (the caller
    /// re-fetches from the API), but loudly: silent misses would hide cache
    /// corruption behind extra API spend.
    pub fn get(&self, key: &CacheKey) -> Option<CachedAiResponse> {
        let path = self.cache_dir.join(key.to_path());

        if !path.exists() {
            return None;
        }
        let content = read_or_warn(&path, fs::read_to_string(&path))?;
        parse_or_warn(&path, &content)
    }

    /// Store a response in the cache.
    pub fn put(&self, key: &CacheKey, response: &ChatResponse, model: &str) -> std::io::Result<()> {
        let path = self.cache_dir.join(key.to_path());

        // Create parent directories
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let cached = CachedAiResponse {
            content: response.content.clone(),
            usage: response.usage.clone(),
            cached_at: Utc::now(),
            model: model.to_string(),
        };

        let json = serde_json::to_string_pretty(&cached)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        fs::write(&path, json)
    }

    /// Remove a cached response. Removing an entry that doesn't exist is a no-op.
    pub fn remove(&self, key: &CacheKey) -> std::io::Result<()> {
        match fs::remove_file(self.cache_dir.join(key.to_path())) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        let mut stats = CacheStats::default();

        if !self.cache_dir.exists() {
            return stats;
        }

        // Count all .json files recursively
        fn count_json_files(dir: &std::path::Path, count: &mut usize) {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_dir() {
                        count_json_files(&path, count);
                    } else if path.extension().is_some_and(|ext| ext == "json") {
                        *count += 1;
                    }
                }
            }
        }

        count_json_files(&self.cache_dir, &mut stats.cached_responses);
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

/// Cache statistics.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub cached_responses: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> CacheKey {
        CacheKey::new("test_prompt", "test/model", &[ChatMessage::user("hi")])
    }

    fn test_response() -> ChatResponse {
        ChatResponse {
            content: "{}".to_string(),
            usage: Usage::default(),
            cached: false,
        }
    }

    #[test]
    fn remove_deletes_cached_entry() {
        let dir = tempfile::TempDir::new().unwrap();
        let cache = AiCache::new(dir.path().to_path_buf());
        let key = test_key();
        cache.put(&key, &test_response(), "test/model").unwrap();
        assert!(cache.get(&key).is_some());

        cache.remove(&key).unwrap();

        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn remove_is_ok_for_missing_entry() {
        let dir = tempfile::TempDir::new().unwrap();
        let cache = AiCache::new(dir.path().to_path_buf());

        cache.remove(&test_key()).unwrap();
    }

    #[test]
    fn corrupt_entry_is_a_cache_miss() {
        let dir = tempfile::TempDir::new().unwrap();
        let cache = AiCache::new(dir.path().to_path_buf());
        let key = test_key();
        cache.put(&key, &test_response(), "test/model").unwrap();
        assert!(cache.get(&key).is_some());

        fs::write(dir.path().join(key.to_path()), "not json").unwrap();

        assert!(cache.get(&key).is_none());
    }
}

/// Compute SHA256 hash and return as hex string.
fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

// We need the hex crate for encoding. Let's use a simple implementation instead.
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes
            .as_ref()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }
}
