//! Shared helpers for disk-cache reads that must fail loudly.
//!
//! Corrupt or unreadable cache entries are treated as misses (callers fall
//! back to the network or API), but with a warning: silent misses would hide
//! cache corruption behind re-fetches and extra spend.

/// Unwrap a cache file read, warning (rather than silently missing) on IO errors.
pub(crate) fn read_or_warn<T>(path: &std::path::Path, result: std::io::Result<T>) -> Option<T> {
    match result {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "failed to read cache file; treating as miss");
            None
        }
    }
}

/// Parse a cache file's JSON, warning (rather than silently missing) on corruption.
pub(crate) fn parse_or_warn<T: serde::de::DeserializeOwned>(
    path: &std::path::Path,
    content: &str,
) -> Option<T> {
    match serde_json::from_str(content) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "corrupt cache file; treating as miss");
            None
        }
    }
}
