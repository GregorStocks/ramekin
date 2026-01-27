//! CLI-specific pipeline step implementations.
//!
//! These implement the `PipelineStep` trait for steps that need CLI-specific behavior
//! (primarily around file I/O instead of database operations).

use std::time::Instant;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;

use ramekin_core::http::HttpClient;
use ramekin_core::pipeline::{
    steps::{FetchImagesStepMeta, SaveRecipeStepMeta},
    PipelineStep, StepContext, StepMetadata, StepResult,
};
use ramekin_core::{fetch_and_validate_image, ExtractRecipeOutput, FailedImageFetch, RawRecipe};

/// CLI implementation of FetchImages step.
///
/// Fetches images from URLs found in the extracted recipe and caches them.
/// Uses the same validation logic as the server (format and size checks).
pub struct FetchImagesStep<C: HttpClient> {
    client: C,
}

impl<C: HttpClient> FetchImagesStep<C> {
    /// Create a new FetchImagesStep with the given HTTP client.
    pub fn new(client: C) -> Self {
        Self { client }
    }
}

#[async_trait]
impl<C: HttpClient + Send + Sync> PipelineStep for FetchImagesStep<C> {
    fn metadata(&self) -> StepMetadata {
        FetchImagesStepMeta::metadata()
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Get extract output to find image URLs
        let extract_output = match ctx.outputs.get_output("extract_recipe") {
            Some(o) => o,
            None => {
                return StepResult {
                    step_name: FetchImagesStepMeta::NAME.to_string(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("extract_recipe output not found".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Parse raw_recipe to get image URLs
        let raw_recipe: RawRecipe = match extract_output
            .get("raw_recipe")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
        {
            Some(r) => r,
            None => {
                return StepResult {
                    step_name: FetchImagesStepMeta::NAME.to_string(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("No raw_recipe in extract output".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Fetch images (only the first one, matching server behavior)
        let mut fetched_urls = Vec::new();
        let mut failed_urls = Vec::new();

        if let Some(url) = raw_recipe.image_urls.first() {
            match fetch_and_validate_image(&self.client, url).await {
                Ok(_fetched) => {
                    // Image is now available (either from cache or freshly fetched)
                    fetched_urls.push(url.clone());
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch image {}: {}", url, e);
                    failed_urls.push(FailedImageFetch {
                        url: url.clone(),
                        error: e,
                    });
                }
            }
        }

        StepResult {
            step_name: FetchImagesStepMeta::NAME.to_string(),
            success: true, // Image fetch failures don't fail the pipeline
            output: json!({
                "fetched_urls": fetched_urls,
                "failed_urls": failed_urls,
                "images_fetched": fetched_urls.len()
            }),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("save_recipe".to_string()),
        }
    }
}

/// CLI implementation of ApplyAutoTags step.
///
/// No-op for CLI - we don't have a database to update.
/// Just passes through to the next step.
pub struct ApplyAutoTagsStep;

impl ApplyAutoTagsStep {
    pub const NAME: &'static str = "apply_auto_tags";
}

#[async_trait]
impl PipelineStep for ApplyAutoTagsStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Apply auto-suggested tags (no-op for CLI)",
            continues_on_failure: true,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Get suggested tags from enrich_auto_tag output for reporting
        let auto_tag_output = ctx.outputs.get_output("enrich_auto_tag");
        let suggested_tags: Vec<String> = auto_tag_output
            .as_ref()
            .and_then(|o| o.get("suggested_tags"))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        StepResult {
            step_name: Self::NAME.to_string(),
            success: true,
            output: json!({
                "message": "No-op for CLI (no database to update)",
                "suggested_tags": suggested_tags,
            }),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("enrich_generate_photo".to_string()),
        }
    }
}

/// CLI implementation of SaveRecipe step.
///
/// Saves the extracted recipe to the output directory as JSON.
pub struct SaveRecipeStep;

#[async_trait]
impl PipelineStep for SaveRecipeStep {
    fn metadata(&self) -> StepMetadata {
        SaveRecipeStepMeta::metadata()
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Get extract output
        let extract_output = match ctx.outputs.get_output("extract_recipe") {
            Some(o) => o,
            None => {
                return StepResult {
                    step_name: SaveRecipeStepMeta::NAME.to_string(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("extract_recipe output not found".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Parse the extract output
        let extract_data: ExtractRecipeOutput = match serde_json::from_value(extract_output) {
            Ok(d) => d,
            Err(e) => {
                return StepResult {
                    step_name: SaveRecipeStepMeta::NAME.to_string(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some(format!("Failed to parse extract output: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Create save output
        let save_output = ramekin_core::SaveRecipeOutput {
            raw_recipe: extract_data.raw_recipe,
            saved_at: Utc::now().to_rfc3339(),
        };

        StepResult {
            step_name: SaveRecipeStepMeta::NAME.to_string(),
            success: true,
            output: serde_json::to_value(&save_output).unwrap_or_default(),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("enrich_normalize_ingredients".to_string()),
        }
    }
}
