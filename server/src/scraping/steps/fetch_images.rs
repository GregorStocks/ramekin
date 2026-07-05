use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use diesel::prelude::*;
use uuid::Uuid;

use ramekin_core::pipeline::{
    steps::FetchImagesStepMeta, PipelineStep, StepContext, StepMetadata, StepResult,
};
use ramekin_core::{FailedImageFetch, FetchImagesOutput, RawRecipe};

use crate::db::DbPool;
use crate::models::NewPhoto;
use crate::photos::processing::{process_image, MAX_FILE_SIZE};
use crate::schema::photos;

use super::super::is_host_allowed;

/// Server implementation of FetchImages step.
///
/// Fetches images from URLs, processes them, and stores as Photo records in the database.
pub struct FetchImagesStep {
    pool: Arc<DbPool>,
    user_id: Uuid,
}

impl FetchImagesStep {
    pub fn new(pool: Arc<DbPool>, user_id: Uuid) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl PipelineStep for FetchImagesStep {
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

        // Fetch images (only the first one for now)
        let mut photo_ids = Vec::new();
        let mut failed_urls = Vec::new();

        if let Some(url) = raw_recipe.image_urls.first() {
            match self.fetch_and_store_image(url).await {
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
            step_name: FetchImagesStepMeta::NAME.to_string(),
            success: true, // Image fetch failures don't fail the pipeline
            output: serde_json::to_value(&output).expect("FetchImagesOutput serializes to JSON"),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("parse_ingredients".to_string()),
        }
    }
}

impl FetchImagesStep {
    async fn fetch_and_store_image(&self, url: &str) -> Result<Uuid, String> {
        // Check host allowlist
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
        let processed = process_image(&data).map_err(|e| e.to_string())?;

        // Store in database
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        let new_photo = NewPhoto {
            user_id: self.user_id,
            content_type: &processed.content_type,
            data: &data,
            thumbnail: &processed.thumbnail,
            width: Some(processed.width as i32),
            height: Some(processed.height as i32),
            file_size: Some(data.len() as i32),
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
