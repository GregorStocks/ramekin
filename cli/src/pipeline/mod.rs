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

use std::sync::Arc;

use ramekin_core::ai::{AiClient, AiConfig, CachingAiClient};
use ramekin_core::http::HttpClient;
use ramekin_core::pipeline::steps::{
    EnrichAutoTagStep, EnrichGenerateDescriptionStep, EnrichNormalizeTitleStep, ExtractRecipeStep,
    FetchHtmlStep, ParseIngredientsStep,
};
use ramekin_core::pipeline::{
    scrape_auto_applied_ai_enrichments, ScrapeAutoAppliedAiEnrichment, StepRegistry,
};

pub use runners::{
    run_all_steps, AllStepsResult, ExtractionStats, IngredientStats, PipelineStep, StepResult,
};
pub use staging::{clear_staging, ensure_staging_dir, find_staged_html, staging_dir};

use steps::{
    ApplyAutoTagsStep, ApplyGeneratedDescriptionStep, ApplyNormalizedTitleStep, FetchImagesStep,
    SaveRecipeStep,
};

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
    registry.register(Box::new(ParseIngredientsStep));
    registry.register(Box::new(SaveRecipeStep));

    for enrichment in scrape_auto_applied_ai_enrichments() {
        match enrichment {
            ScrapeAutoAppliedAiEnrichment::NormalizeTitle => {
                let mut ai_config =
                    AiConfig::from_env().expect("OPENROUTER_API_KEY must be set in cli.env");
                ai_config.rate_limit_ms = 0;
                let ai_client: Arc<dyn AiClient> = Arc::new(CachingAiClient::new(ai_config));
                registry.register(Box::new(EnrichNormalizeTitleStep::new(ai_client)));
                registry.register(Box::new(ApplyNormalizedTitleStep));
            }
            ScrapeAutoAppliedAiEnrichment::GenerateDescription => {
                let mut ai_config =
                    AiConfig::from_env().expect("OPENROUTER_API_KEY must be set in cli.env");
                ai_config.rate_limit_ms = 0;
                let ai_client: Arc<dyn AiClient> = Arc::new(CachingAiClient::new(ai_config));
                registry.register(Box::new(EnrichGenerateDescriptionStep::new(ai_client)));
                registry.register(Box::new(ApplyGeneratedDescriptionStep));
            }
            ScrapeAutoAppliedAiEnrichment::AutoTag => {
                let mut ai_config =
                    AiConfig::from_env().expect("OPENROUTER_API_KEY must be set in cli.env");
                ai_config.rate_limit_ms = 0;
                let ai_client: Arc<dyn AiClient> = Arc::new(CachingAiClient::new(ai_config));
                registry.register(Box::new(EnrichAutoTagStep::new(
                    ai_client,
                    user_tags.clone(),
                )));
                registry.register(Box::new(ApplyAutoTagsStep));
            }
        }
    }

    registry
}
