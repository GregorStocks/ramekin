//! CLI pipeline module.
//!
//! This module provides:
//! - Generic pipeline integration via `build_registry` and `FileOutputStore`
//! - Step runners for the orchestrator (run_fetch_html, run_all_steps, etc.)
//! - Staging utilities for manual HTML caching

mod output_store;
mod runners;
pub mod snapshots;
mod staging;
mod steps;

use ramekin_core::http::HttpClient;
use ramekin_core::pipeline::steps::{ExtractRecipeStep, FetchHtmlStep, ParseIngredientsStep};
use ramekin_core::pipeline::StepRegistry;

pub use runners::{
    run_all_steps, AllStepsResult, ExtractionStats, IngredientStats, PipelineStep, StepResult,
};
pub use staging::{clear_staging, ensure_staging_dir, find_staged_html, staging_dir};

use steps::{FetchImagesStep, SaveRecipeStep};

/// Build a step registry with all CLI pipeline steps.
///
/// The HTTP client is injected for fetch_html and fetch_images steps.
pub fn build_registry<C: HttpClient + Clone + Send + Sync + 'static>(client: C) -> StepRegistry {
    let mut registry = StepRegistry::new();

    registry.register(Box::new(FetchHtmlStep::new(client.clone())));
    registry.register(Box::new(ExtractRecipeStep));
    registry.register(Box::new(FetchImagesStep::new(client)));
    registry.register(Box::new(ParseIngredientsStep));
    registry.register(Box::new(SaveRecipeStep));

    registry
}
