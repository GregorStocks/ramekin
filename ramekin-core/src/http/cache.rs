//! Disk-based HTTP response cache with ETag support.
//!
//! Each URL gets a directory holding either a successful response
//! (`response.bin` + `metadata.json`), a negative-cache entry (`error.txt`),
//! both, or leftovers from an interrupted write. [`DiskCache::classify`] is the
//! one place that decides what such a directory means.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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

/// Whether a cached HTTP error should keep being served instead of refetching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorPermanence {
    /// The URL itself is gone (404/410), so caching the failure is safe.
    Permanent,
    /// Depends on who's asking or when (e.g. a 403 User-Agent block or a
    /// 429). Never served from cache, so the URL is retried.
    Transient,
}

impl ErrorPermanence {
    pub fn from_status(status: reqwest::StatusCode) -> Self {
        if matches!(
            status,
            reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE
        ) {
            Self::Permanent
        } else {
            Self::Transient
        }
    }

    /// Entries written before the typed marker existed (and by older builds
    /// in other worktrees sharing the same cache directory) only carry the
    /// `format!("HTTP {status}")` message, so derive permanence from it.
    fn from_legacy_message(error: &str) -> Self {
        if error.starts_with("HTTP 404 ") || error.starts_with("HTTP 410 ") {
            Self::Permanent
        } else {
            Self::Transient
        }
    }
}

/// A cached error response.
#[derive(Debug, Clone)]
pub struct CachedError {
    pub error: String,
    pub fetched_at: DateTime<Utc>,
    pub permanence: ErrorPermanence,
}

/// On-disk shape of `error.txt`. `permanence` is optional so legacy entries
/// still read; new entries always write it.
#[derive(Serialize, Deserialize)]
struct StoredError {
    error: String,
    fetched_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    permanence: Option<ErrorPermanence>,
}

impl From<StoredError> for CachedError {
    fn from(stored: StoredError) -> Self {
        let permanence = stored
            .permanence
            .unwrap_or_else(|| ErrorPermanence::from_legacy_message(&stored.error));
        Self {
            error: stored.error,
            fetched_at: stored.fetched_at,
            permanence,
        }
    }
}

/// Result of a cache lookup.
#[derive(Debug, Clone)]
pub enum CacheLookup {
    /// A complete cached response.
    Hit(CachedResponse),
    /// A permanent cached failure, with no good response to fall back on.
    Error(CachedError),
    /// Nothing servable; the caller should fetch.
    Miss,
}

/// What's on disk for one URL, as decided by [`DiskCache::classify`].
#[derive(Debug, Clone)]
enum EntryState {
    /// No cache files.
    Absent,
    /// `response.bin` plus readable `metadata.json`. Takes precedence over any
    /// `error.txt`, so a cached error never hides a good copy.
    Success(CacheMetadata),
    /// A permanent negative-cache entry with no good response.
    PermanentError(CachedError),
    /// Files exist but nothing is servable: a response without metadata (an
    /// interrupted write), corrupt files, or a transient error. Treated as a
    /// miss everywhere.
    Unusable,
}

/// Cache statistics.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub cached_success: usize,
    pub cached_errors: usize,
    /// Entries with files on disk that are treated as misses.
    pub unusable: usize,
}

const RESPONSE_FILE: &str = "response.bin";
const METADATA_FILE: &str = "metadata.json";
const ERROR_FILE: &str = "error.txt";

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

    /// The single source of truth for what a cache directory holds. Every
    /// reader goes through this, so `lookup`, `get_error`, `is_cached` and
    /// `stats` can't disagree about an entry.
    ///
    /// Corrupt or unreadable files are treated as misses, but loudly: silent
    /// misses would hide cache corruption behind re-fetches.
    fn classify(dir: &Path) -> EntryState {
        let response_path = dir.join(RESPONSE_FILE);
        let metadata_path = dir.join(METADATA_FILE);
        let error_path = dir.join(ERROR_FILE);

        let response_exists = response_path.exists();
        let metadata_exists = metadata_path.exists();
        if response_exists && metadata_exists {
            let metadata = read_or_warn(&metadata_path, fs::read_to_string(&metadata_path))
                .and_then(|s| parse_or_warn::<CacheMetadata>(&metadata_path, &s));
            if let Some(metadata) = metadata {
                return EntryState::Success(metadata);
            }
        }

        if error_path.exists() {
            let cached = read_or_warn(&error_path, fs::read_to_string(&error_path))
                .and_then(|s| parse_or_warn::<StoredError>(&error_path, &s))
                .map(CachedError::from);
            return match cached {
                Some(e) if e.permanence == ErrorPermanence::Permanent => {
                    EntryState::PermanentError(e)
                }
                _ => EntryState::Unusable,
            };
        }

        if response_exists || metadata_exists {
            EntryState::Unusable
        } else {
            EntryState::Absent
        }
    }

    /// Look up a URL. A good response wins over a cached error; anything
    /// else is a miss.
    pub fn lookup(&self, url: &str) -> CacheLookup {
        let dir = self.url_dir(url);
        match Self::classify(&dir) {
            EntryState::Success(metadata) => {
                let response_path = dir.join(RESPONSE_FILE);
                match read_or_warn(&response_path, fs::read(&response_path)) {
                    Some(data) => CacheLookup::Hit(CachedResponse { data, metadata }),
                    None => CacheLookup::Miss,
                }
            }
            EntryState::PermanentError(e) => CacheLookup::Error(e),
            EntryState::Absent | EntryState::Unusable => CacheLookup::Miss,
        }
    }

    /// Check if a URL has a servable entry (a response or a permanent error).
    pub fn is_cached(&self, url: &str) -> bool {
        matches!(
            Self::classify(&self.url_dir(url)),
            EntryState::Success(_) | EntryState::PermanentError(_)
        )
    }

    /// Get cached response if it exists.
    pub fn get(&self, url: &str) -> Option<CachedResponse> {
        match self.lookup(url) {
            CacheLookup::Hit(r) => Some(r),
            CacheLookup::Error(_) | CacheLookup::Miss => None,
        }
    }

    /// Get the cached permanent error, if there is one and no good response.
    ///
    /// Only permanent errors (404/410) count. Older caches also stored 403s
    /// and 429s, which depend on who's asking (e.g. a User-Agent block) and
    /// would otherwise never be retried, so those are treated as misses.
    pub fn get_error(&self, url: &str) -> Option<CachedError> {
        match Self::classify(&self.url_dir(url)) {
            EntryState::PermanentError(e) => Some(e),
            _ => None,
        }
    }

    /// Get cache metadata without loading the full response.
    pub fn get_metadata(&self, url: &str) -> Option<CacheMetadata> {
        match Self::classify(&self.url_dir(url)) {
            EntryState::Success(metadata) => Some(metadata),
            _ => None,
        }
    }

    /// Save a successful response to the cache.
    ///
    /// Each file is written atomically (temp file + rename), and
    /// `metadata.json` goes last, so a crash mid-write leaves at worst a new
    /// response without metadata, which `classify` treats as a miss.
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

        write_atomic(&dir, RESPONSE_FILE, data)?;
        write_atomic(
            &dir,
            METADATA_FILE,
            serde_json::to_string_pretty(&metadata).unwrap().as_bytes(),
        )?;

        // A good response already wins over a cached error; drop the stale
        // error file so the entry is unambiguous on disk.
        let error_path = dir.join(ERROR_FILE);
        if error_path.exists() {
            let _ = fs::remove_file(error_path);
        }

        Ok(())
    }

    /// Save an error to the cache (negative caching).
    ///
    /// Leaves any existing good response in place; `classify` prefers it.
    pub fn put_error(
        &self,
        url: &str,
        error: &str,
        permanence: ErrorPermanence,
    ) -> std::io::Result<()> {
        let dir = self.url_dir(url);
        fs::create_dir_all(&dir)?;

        let stored = StoredError {
            error: error.to_string(),
            fetched_at: Utc::now(),
            permanence: Some(permanence),
        };

        write_atomic(
            &dir,
            ERROR_FILE,
            serde_json::to_string_pretty(&stored).unwrap().as_bytes(),
        )
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        let mut stats = CacheStats::default();

        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                match Self::classify(&path) {
                    EntryState::Success(_) => stats.cached_success += 1,
                    EntryState::PermanentError(_) => stats.cached_errors += 1,
                    EntryState::Unusable => stats.unusable += 1,
                    EntryState::Absent => {}
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

/// Write `dir/name` atomically: readers see the old file or the new one,
/// never a partial write. The temp file lives in `dir` so the rename stays on
/// one filesystem, and its random name keeps concurrent writers (other
/// worktrees share the cache) from clobbering each other's temp files.
fn write_atomic(dir: &Path, name: &str, data: &[u8]) -> std::io::Result<()> {
    let mut tmp = tempfile::Builder::new()
        .prefix(&format!(".{name}."))
        .suffix(".tmp")
        .tempfile_in(dir)?;
    tmp.write_all(data)?;
    tmp.persist(dir.join(name)).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://example.com/recipe";

    fn entry_dir(root: &Path) -> PathBuf {
        root.join(slugify_url(URL))
    }

    #[test]
    fn corrupt_metadata_is_a_cache_miss() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf());
        cache.put(URL, b"body", None, None, None).unwrap();
        assert!(cache.get(URL).is_some());

        fs::write(entry_dir(dir.path()).join(METADATA_FILE), "not json").unwrap();

        assert!(cache.get(URL).is_none());
        assert!(cache.get_metadata(URL).is_none());
        assert!(!cache.is_cached(URL));
    }

    #[test]
    fn corrupt_cached_error_is_a_cache_miss() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf());
        cache
            .put_error(URL, "HTTP 404 Not Found", ErrorPermanence::Permanent)
            .unwrap();
        assert!(cache.get_error(URL).is_some());

        fs::write(entry_dir(dir.path()).join(ERROR_FILE), "not json").unwrap();

        assert!(cache.get_error(URL).is_none());
        assert!(!cache.is_cached(URL));
    }

    #[test]
    fn transient_cached_error_is_a_cache_miss() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf());

        cache
            .put_error(URL, "HTTP 403 Forbidden", ErrorPermanence::Transient)
            .unwrap();
        assert!(cache.get_error(URL).is_none());
        assert!(!cache.is_cached(URL));
        assert!(matches!(cache.lookup(URL), CacheLookup::Miss));

        cache
            .put_error(URL, "HTTP 410 Gone", ErrorPermanence::Permanent)
            .unwrap();
        assert!(cache.get_error(URL).is_some());
        assert!(cache.is_cached(URL));
        assert!(matches!(cache.lookup(URL), CacheLookup::Error(_)));
    }

    #[test]
    fn typed_marker_wins_over_message() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf());
        cache
            .put_error(URL, "HTTP 404 Not Found", ErrorPermanence::Transient)
            .unwrap();
        assert!(cache.get_error(URL).is_none());

        cache
            .put_error(URL, "something else", ErrorPermanence::Permanent)
            .unwrap();
        assert_eq!(
            cache.get_error(URL).unwrap().permanence,
            ErrorPermanence::Permanent
        );
    }

    /// Entries written by older builds (still running in other worktrees
    /// that share the cache) have no `permanence` field.
    #[test]
    fn legacy_error_entries_are_classified_by_message() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf());
        let entry = entry_dir(dir.path());
        fs::create_dir_all(&entry).unwrap();

        let legacy = |error: &str| {
            format!(r#"{{"error": "{error}", "fetched_at": "2026-05-13T23:09:14.999214Z"}}"#)
        };

        for (msg, permanent) in [
            ("HTTP 404 Not Found", true),
            ("HTTP 410 Gone", true),
            ("HTTP 403 Forbidden", false),
            ("HTTP 429 Too Many Requests", false),
        ] {
            fs::write(entry.join(ERROR_FILE), legacy(msg)).unwrap();
            assert_eq!(cache.get_error(URL).is_some(), permanent, "{msg}");
            assert_eq!(cache.is_cached(URL), permanent, "{msg}");
        }
    }

    #[test]
    fn new_error_entries_stay_readable_by_legacy_shape() {
        // Older builds deserialize `{error, fetched_at}` and ignore unknown
        // fields, and match on the message; keep both intact.
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf());
        cache
            .put_error(URL, "HTTP 404 Not Found", ErrorPermanence::Permanent)
            .unwrap();
        let raw: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(entry_dir(dir.path()).join(ERROR_FILE)).unwrap(),
        )
        .unwrap();
        assert_eq!(raw["error"], "HTTP 404 Not Found");
        assert!(raw["fetched_at"].is_string());
        assert_eq!(raw["permanence"], "permanent");
    }

    #[test]
    fn good_response_wins_over_cached_error() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf());
        cache.put(URL, b"body", None, None, None).unwrap();
        cache
            .put_error(URL, "HTTP 404 Not Found", ErrorPermanence::Permanent)
            .unwrap();

        assert!(cache.get_error(URL).is_none());
        match cache.lookup(URL) {
            CacheLookup::Hit(r) => assert_eq!(r.data, b"body"),
            other => panic!("expected hit, got {other:?}"),
        }
        let stats = cache.stats();
        assert_eq!((stats.cached_success, stats.cached_errors), (1, 0));
    }

    #[test]
    fn put_clears_cached_error() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf());
        cache
            .put_error(URL, "HTTP 404 Not Found", ErrorPermanence::Permanent)
            .unwrap();
        cache.put(URL, b"body", None, None, None).unwrap();
        assert!(!entry_dir(dir.path()).join(ERROR_FILE).exists());
        assert!(cache.get(URL).is_some());
    }

    #[test]
    fn response_without_metadata_is_a_miss_everywhere() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf());
        let entry = entry_dir(dir.path());
        fs::create_dir_all(&entry).unwrap();
        fs::write(entry.join(RESPONSE_FILE), b"half-written").unwrap();

        assert!(cache.get(URL).is_none());
        assert!(!cache.is_cached(URL));
        let stats = cache.stats();
        assert_eq!(
            (stats.cached_success, stats.cached_errors, stats.unusable),
            (0, 0, 1)
        );
    }

    #[test]
    fn partial_response_falls_back_to_permanent_error() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf());
        cache
            .put_error(URL, "HTTP 404 Not Found", ErrorPermanence::Permanent)
            .unwrap();
        fs::write(entry_dir(dir.path()).join(RESPONSE_FILE), b"half-written").unwrap();

        assert!(cache.get_error(URL).is_some());
        assert!(cache.is_cached(URL));
        assert_eq!(cache.stats().cached_errors, 1);
    }

    #[test]
    fn stats_and_is_cached_agree_with_lookup() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf());
        let urls = [
            "https://example.com/ok",
            "https://example.com/gone",
            "https://example.com/blocked",
        ];
        cache.put(urls[0], b"body", None, None, None).unwrap();
        cache
            .put_error(urls[1], "HTTP 410 Gone", ErrorPermanence::Permanent)
            .unwrap();
        cache
            .put_error(urls[2], "HTTP 403 Forbidden", ErrorPermanence::Transient)
            .unwrap();
        // An empty leftover directory is absent, not unusable.
        fs::create_dir_all(dir.path().join("empty-dir")).unwrap();

        let stats = cache.stats();
        assert_eq!(
            (stats.cached_success, stats.cached_errors, stats.unusable),
            (1, 1, 1)
        );
        assert!(cache.is_cached(urls[0]));
        assert!(cache.is_cached(urls[1]));
        assert!(!cache.is_cached(urls[2]));
    }

    #[test]
    fn writes_leave_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf());
        cache.put(URL, b"one", None, None, None).unwrap();
        cache.put(URL, b"two", None, None, None).unwrap();
        cache
            .put_error(URL, "HTTP 404 Not Found", ErrorPermanence::Permanent)
            .unwrap();
        let mut names: Vec<String> = fs::read_dir(entry_dir(dir.path()))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, [ERROR_FILE, METADATA_FILE, RESPONSE_FILE]);
        assert_eq!(cache.get(URL).unwrap().data, b"two");
    }

    #[test]
    fn permanence_from_status() {
        use reqwest::StatusCode;
        assert_eq!(
            ErrorPermanence::from_status(StatusCode::NOT_FOUND),
            ErrorPermanence::Permanent
        );
        assert_eq!(
            ErrorPermanence::from_status(StatusCode::GONE),
            ErrorPermanence::Permanent
        );
        for s in [
            StatusCode::FORBIDDEN,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
        ] {
            assert_eq!(ErrorPermanence::from_status(s), ErrorPermanence::Transient);
        }
    }
}
