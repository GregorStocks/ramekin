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

use std::sync::Arc;

use ramekin_core::ai::{AiClient, CachingAiClient};
use ramekin_core::http::HttpClient;
use ramekin_core::pipeline::steps::{
    EnrichAutoTagStep, EnrichGeneratePhotoStep, EnrichNormalizeIngredientsStep, ExtractRecipeStep,
    FetchHtmlStep,
};
use ramekin_core::pipeline::StepRegistry;

pub use runners::{run_all_steps, AllStepsResult, ExtractionStats, PipelineStep, StepResult};
pub use staging::{clear_staging, ensure_staging_dir, find_staged_html, staging_dir};

use steps::{ApplyAutoTagsStep, FetchImagesStep, SaveRecipeStep};

/// Build a step registry with all CLI pipeline steps.
///
/// The HTTP client is injected for fetch_html and fetch_images steps.
/// The AI client is created from environment variables.
/// User tags are used for auto-tagging evaluation.
pub fn build_registry<C: HttpClient + Clone + Send + Sync + 'static>(
    client: C,
    user_tags: Vec<String>,
) -> StepRegistry {
    let mut registry = StepRegistry::new();

    registry.register(Box::new(FetchHtmlStep::new(client.clone())));
    registry.register(Box::new(ExtractRecipeStep));
    registry.register(Box::new(FetchImagesStep::new(client)));
    registry.register(Box::new(SaveRecipeStep));
    registry.register(Box::new(EnrichNormalizeIngredientsStep));

    // Create AI client for auto-tagging
    let ai_client: Arc<dyn AiClient> =
        Arc::new(CachingAiClient::from_env().expect("OPENROUTER_API_KEY must be set in cli.env"));
    registry.register(Box::new(EnrichAutoTagStep::new(ai_client, user_tags)));
    registry.register(Box::new(ApplyAutoTagsStep));

    registry.register(Box::new(EnrichGeneratePhotoStep));

    registry
}
