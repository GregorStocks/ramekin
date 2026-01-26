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
pub struct FileOutputStore {
    run_dir: PathBuf,
    url_slug: String,
    /// In-memory cache of outputs for fast access during pipeline execution
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
}

impl StepOutputStore for FileOutputStore {
    fn get_output(&self, step_name: &str) -> Option<JsonValue> {
        // Check in-memory cache first
        if let Some(cached) = self.cache.get(step_name) {
            return Some(cached.clone());
        }

        // Try to load from disk
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
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Save to in-memory cache
        self.cache.insert(step_name.to_string(), output.clone());

        // Save to disk
        let dir = self.step_dir(step_name);
        fs::create_dir_all(&dir)?;

        let json = serde_json::to_string_pretty(output)?;
        fs::write(self.output_path(step_name), json)?;

        Ok(())
    }
}
