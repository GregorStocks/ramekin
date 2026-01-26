//! CLI pipeline module.
//!
//! This module provides:
//! - Generic pipeline integration via `build_registry` and `FileOutputStore`
//! - Step runners for the orchestrator (run_fetch_html, run_all_steps, etc.)
//! - Staging utilities for manual HTML caching

mod output_store;
mod runners;
mod staging;
mod steps;

use ramekin_core::http::HttpClient;
use ramekin_core::pipeline::steps::{
    EnrichAutoTagStep, EnrichGeneratePhotoStep, EnrichNormalizeIngredientsStep, ExtractRecipeStep,
    FetchHtmlStep,
};
use ramekin_core::pipeline::StepRegistry;

pub use runners::{run_all_steps, AllStepsResult, ExtractionStats, PipelineStep, StepResult};
pub use staging::{clear_staging, ensure_staging_dir, find_staged_html, staging_dir};

use steps::{FetchImagesStep, SaveRecipeStep};

/// Build a step registry with all CLI pipeline steps.
///
/// The HTTP client is injected for the fetch_html step.
pub fn build_registry<C: HttpClient + Send + Sync + 'static>(client: C) -> StepRegistry {
    let mut registry = StepRegistry::new();

    registry.register(Box::new(FetchHtmlStep::new(client)));
    registry.register(Box::new(ExtractRecipeStep));
    registry.register(Box::new(FetchImagesStep));
    registry.register(Box::new(SaveRecipeStep));
    registry.register(Box::new(EnrichNormalizeIngredientsStep));
    registry.register(Box::new(EnrichAutoTagStep));
    registry.register(Box::new(EnrichGeneratePhotoStep));

    registry
}
