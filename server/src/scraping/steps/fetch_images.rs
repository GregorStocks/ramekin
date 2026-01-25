//! Server FetchImages step - downloads images and stores as Photo records.

use std::time::Instant;

use async_trait::async_trait;
use diesel::prelude::*;
use ramekin_core::pipeline::steps::FetchImagesStepMeta;
use ramekin_core::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};
use ramekin_core::{FailedImageFetch, FetchImagesOutput, RawRecipe};
use serde_json::json;
use uuid::Uuid;

use crate::db::DbPool;
use crate::models::NewPhoto;
use crate::photos::processing::{process_image, MAX_FILE_SIZE};
use crate::schema::photos;
use crate::scraping::is_host_allowed;

/// Server implementation of FetchImages - downloads images and stores as Photo records.
pub struct FetchImagesStep<'a> {
    pool: &'a DbPool,
    user_id: Uuid,
}

impl<'a> FetchImagesStep<'a> {
    /// Create a new FetchImagesStep.
    pub fn new(pool: &'a DbPool, user_id: Uuid) -> Self {
        Self { pool, user_id }
    }

    /// Fetch a single image, process it, and store as a Photo record.
    async fn fetch_and_store_single_image(&self, url: &str) -> Result<Uuid, String> {
        // Check if host is allowed
        is_host_allowed(url).map_err(|e| e.to_string())?;

        // Fetch the image bytes
        let data = ramekin_core::fetch_bytes(url)
            .await
            .map_err(|e| e.to_string())?;

        // Validate size
        if data.len() > MAX_FILE_SIZE {
            return Err(format!(
                "Image too large: {} bytes (max {})",
                data.len(),
                MAX_FILE_SIZE
            ));
        }

        // Process: validate format, generate thumbnail
        let (content_type, thumbnail) = process_image(&data)?;

        // Store in database
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        let new_photo = NewPhoto {
            user_id: self.user_id,
            content_type: &content_type,
            data: &data,
            thumbnail: &thumbnail,
        };

        let photo_id: Uuid = diesel::insert_into(photos::table)
            .values(&new_photo)
            .returning(photos::id)
            .get_result(&mut conn)
            .map_err(|e| e.to_string())?;

        tracing::info!("Stored photo {} from {}", photo_id, url);
        Ok(photo_id)
    }
}

#[async_trait]
impl PipelineStep for FetchImagesStep<'_> {
    fn metadata(&self) -> StepMetadata {
        FetchImagesStepMeta::metadata()
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Get raw_recipe from extract output to get image URLs
        let extract_output = match ctx.outputs.get_output("extract_recipe") {
            Some(o) => o,
            None => {
                return StepResult {
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("extract_recipe output not found".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        let raw_recipe: RawRecipe = match extract_output.get("raw_recipe") {
            Some(v) => match serde_json::from_value(v.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return StepResult {
                        success: false,
                        output: serde_json::Value::Null,
                        error: Some(format!("Failed to parse raw_recipe: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        next_step: None,
                    };
                }
            },
            None => {
                return StepResult {
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("No raw_recipe in extract_recipe output".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Fetch images (only the first one for now)
        let mut photo_ids = Vec::new();
        let mut failed_urls = Vec::new();

        if let Some(url) = raw_recipe.image_urls.first() {
            match self.fetch_and_store_single_image(url).await {
                Ok(photo_id) => photo_ids.push(photo_id),
                Err(e) => {
                    tracing::warn!("Failed to fetch image {}: {}", url, e);
                    failed_urls.push(FailedImageFetch {
                        url: url.clone(),
                        error: e,
                    });
                }
            }
        }

        let output = FetchImagesOutput {
            photo_ids,
            failed_urls,
        };

        StepResult {
            success: true, // Image fetch failures are not fatal
            output: json!(output),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("save_recipe".to_string()),
        }
    }
}
