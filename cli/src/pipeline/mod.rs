//! CLI pipeline module - FileOutputStore and registry building.
//!
//! This module provides the CLI-specific implementations for the generic
//! pipeline framework. It includes:
//! - FileOutputStore: stores step outputs as JSON files on disk
//! - build_registry: assembles CLI-specific step implementations
//! - run_all_steps: convenience function to run the full pipeline

// Infrastructure for future generic pipeline integration.
// Currently unused while legacy pipeline code remains in use.
#![allow(dead_code)]

pub mod steps;

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use ramekin_core::http::{slugify_url, CachingClient};
use ramekin_core::pipeline::steps::{EnrichStep, ExtractRecipeStep, FetchHtmlStep};
use ramekin_core::pipeline::{run_pipeline, StepOutputStore, StepRegistry, StepResult};
use serde_json::Value as JsonValue;

use self::steps::{FetchImagesNoOp, SaveRecipeStep};

/// File-based output store for CLI pipeline.
///
/// Stores step outputs in a directory structure:
/// `{run_dir}/urls/{url_slug}/{step_name}/output.json`
pub struct FileOutputStore {
    run_dir: PathBuf,
    url_slug: String,
}

impl FileOutputStore {
    /// Create a new FileOutputStore.
    pub fn new(run_dir: &Path, url_slug: String) -> Self {
        Self {
            run_dir: run_dir.to_path_buf(),
            url_slug,
        }
    }

    fn step_dir(&self, step_name: &str) -> PathBuf {
        self.run_dir
            .join("urls")
            .join(&self.url_slug)
            .join(step_name)
    }
}

impl StepOutputStore for FileOutputStore {
    fn get_output(&self, step_name: &str) -> Option<JsonValue> {
        let path = self.step_dir(step_name).join("output.json");
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    }

    fn save_output(
        &mut self,
        step_name: &str,
        output: &JsonValue,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let dir = self.step_dir(step_name);
        fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(output)?;
        fs::write(dir.join("output.json"), json)?;
        Ok(())
    }
}

/// Build a step registry for CLI pipeline execution.
///
/// This includes:
/// - FetchHtmlStep with CachingClient (for HTTP caching)
/// - ExtractRecipeStep (generic)
/// - FetchImagesNoOp (CLI-specific: skips image fetching)
/// - SaveRecipeStep (CLI-specific: saves to file)
/// - EnrichStep (generic: always fails, continues_on_failure)
pub fn build_registry(client: CachingClient) -> StepRegistry {
    let mut registry = StepRegistry::new();
    registry.register(Box::new(FetchHtmlStep::new(client)));
    registry.register(Box::new(ExtractRecipeStep));
    registry.register(Box::new(FetchImagesNoOp));
    registry.register(Box::new(SaveRecipeStep));
    registry.register(Box::new(EnrichStep));
    registry
}

/// Run all pipeline steps for a URL using the new generic framework.
///
/// This is a convenience function that:
/// 1. Creates a FileOutputStore
/// 2. Builds the CLI registry
/// 3. Runs the pipeline from fetch_html
/// 4. Returns the results
pub async fn run_all_steps_new(
    url: &str,
    client: CachingClient,
    run_dir: &Path,
) -> Vec<StepResult> {
    let mut store = FileOutputStore::new(run_dir, slugify_url(url));
    let registry = build_registry(client);

    run_pipeline("fetch_html", url, &mut store, &registry).await
}
