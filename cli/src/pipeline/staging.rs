//! Staging directory utilities for manual HTML caching.

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

/// Get the staging directory path for manual HTML saves.
pub fn staging_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".ramekin").join("cache-staging"))
        .unwrap_or_else(|| PathBuf::from("data/cache-staging"))
}

/// Ensure staging directory exists and return its path.
pub fn ensure_staging_dir() -> Result<PathBuf> {
    let staging = staging_dir();
    fs::create_dir_all(&staging)?;
    Ok(staging)
}

/// Find the newest .html file in staging directory.
pub fn find_staged_html() -> Option<PathBuf> {
    let staging = staging_dir();
    if !staging.exists() {
        return None;
    }

    fs::read_dir(&staging)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "html")
                .unwrap_or(false)
                || e.path()
                    .extension()
                    .map(|ext| ext == "htm")
                    .unwrap_or(false)
        })
        .max_by_key(|e| e.metadata().ok().and_then(|m| m.modified().ok()))
        .map(|e| e.path())
}

/// Clear any existing files in staging directory.
pub fn clear_staging() -> Result<()> {
    let staging = staging_dir();
    if staging.exists() {
        for entry in fs::read_dir(&staging)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                fs::remove_file(&path)?;
            }
        }
    }
    Ok(())
}
