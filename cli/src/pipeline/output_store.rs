//! File-based step output store for the CLI.

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use ramekin_core::http::slugify_url;
use ramekin_core::pipeline::StepOutputStore;
use serde_json::Value as JsonValue;

/// File-based output store for CLI pipeline runs.
///
/// Stores step outputs as JSON files in a directory structure:
/// `run_dir/urls/{url_slug}/{step_name}/output.json`
///
/// Also caches outputs in memory to avoid redundant disk reads.
pub struct FileOutputStore {
    run_dir: PathBuf,
    url_slug: String,
    /// In-memory cache to avoid disk round-trips
    cache: HashMap<String, JsonValue>,
}

impl FileOutputStore {
    /// Create a new file output store for a URL.
    pub fn new(run_dir: &Path, url: &str) -> Self {
        Self {
            run_dir: run_dir.to_path_buf(),
            url_slug: slugify_url(url),
            cache: HashMap::new(),
        }
    }

    /// Get the output directory for a step.
    fn step_dir(&self, step_name: &str) -> PathBuf {
        self.run_dir
            .join("urls")
            .join(&self.url_slug)
            .join(step_name)
    }

    /// Get the output file path for a step.
    fn output_path(&self, step_name: &str) -> PathBuf {
        self.step_dir(step_name).join("output.json")
    }

    /// Cache output in memory only (skip disk write).
    /// Useful for large data that's already persisted elsewhere (e.g., HTML in disk cache).
    pub fn cache_only(&mut self, step_name: &str, output: JsonValue) {
        self.cache.insert(step_name.to_string(), output);
    }
}

impl StepOutputStore for FileOutputStore {
    fn get_output(&self, step_name: &str) -> Option<JsonValue> {
        // Check in-memory cache first
        if let Some(value) = self.cache.get(step_name) {
            return Some(value.clone());
        }

        // Fall back to disk
        let path = self.output_path(step_name);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(value) = serde_json::from_str(&content) {
                    return Some(value);
                }
            }
        }
        None
    }

    fn save_output(
        &mut self,
        step_name: &str,
        output: &JsonValue,
        duration_ms: i64,
        success: bool,
        error: Option<&str>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let dir = self.step_dir(step_name);
        fs::create_dir_all(&dir)?;

        // Failures go to error.json so that output.json (what get_output and
        // the snapshot writer read) only ever holds successful output.
        if !success {
            let json = serde_json::to_string_pretty(&serde_json::json!({
                "error": error,
                "output": output,
                "duration_ms": duration_ms,
            }))?;
            fs::write(dir.join("error.json"), json)?;
            return Ok(());
        }

        // Cache in memory
        self.cache.insert(step_name.to_string(), output.clone());

        let json = serde_json::to_string_pretty(output)?;
        fs::write(self.output_path(step_name), json)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn successful_output_is_written_and_readable() {
        let dir = TempDir::new().unwrap();
        let mut store = FileOutputStore::new(dir.path(), "https://example.com/recipe");

        store
            .save_output("extract_recipe", &json!({ "value": 42 }), 5, true, None)
            .unwrap();

        assert_eq!(
            store.get_output("extract_recipe"),
            Some(json!({ "value": 42 }))
        );
        assert!(store.output_path("extract_recipe").exists());
        assert!(!store.step_dir("extract_recipe").join("error.json").exists());
    }

    #[test]
    fn failed_output_goes_to_error_json_and_is_not_readable() {
        let dir = TempDir::new().unwrap();
        let mut store = FileOutputStore::new(dir.path(), "https://example.com/recipe");

        store
            .save_output(
                "enrich_normalize_title",
                &json!({ "error": "AI call failed: details" }),
                7,
                false,
                Some("AI call failed: details"),
            )
            .unwrap();

        // get_output must keep returning None for failed steps so downstream
        // steps and the snapshot writer never consume error payloads.
        assert_eq!(store.get_output("enrich_normalize_title"), None);
        assert!(!store.output_path("enrich_normalize_title").exists());

        let error_path = store.step_dir("enrich_normalize_title").join("error.json");
        let persisted: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(error_path).unwrap()).unwrap();
        assert_eq!(
            persisted,
            json!({
                "error": "AI call failed: details",
                "output": { "error": "AI call failed: details" },
                "duration_ms": 7,
            })
        );
    }
}
