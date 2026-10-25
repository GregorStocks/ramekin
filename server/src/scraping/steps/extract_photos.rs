use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;

use ramekin_core::ai::{extract_recipe_from_photos, AiConfig, CachingAiClient};
use ramekin_core::pipeline::steps::ExtractRecipeStep;
use ramekin_core::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};
use ramekin_core::{ExtractRecipeOutput, ExtractionMethod};

use crate::db::DbPool;
use crate::ids::Uuid;
use crate::photos::load_photo_images;

use super::super::jobs::{source_photo_ids, SOURCE_PHOTOS_STEP};
use super::helpers::step_failure;

/// `extract_recipe` for a recipe imported from photos: reads the photos
/// again with the vision model, as the import did.
pub struct ExtractPhotosStep {
    pub pool: Arc<DbPool>,
    pub user_id: Uuid,
}

#[async_trait]
impl PipelineStep for ExtractPhotosStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: ExtractRecipeStep::NAME,
            description: "Extract the recipe from its photos",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();
        let fail = |error: String| step_failure(ExtractRecipeStep::NAME, start, error);
        let Some(photo_ids) = ctx
            .outputs
            .get_output(SOURCE_PHOTOS_STEP)
            .await
            .and_then(|output| source_photo_ids(&output))
        else {
            return fail("source_photos output not found".to_string());
        };
        let images = match load_photo_images(&self.pool, self.user_id, &photo_ids).await {
            Ok(images) => images,
            Err(e) => return fail(format!("Couldn't load the photos: {e}")),
        };
        let client = match AiConfig::from_env() {
            Ok(config) => CachingAiClient::new(config.for_extraction()),
            Err(e) => return fail(format!("AI is not configured: {e}")),
        };
        let extracted = match extract_recipe_from_photos(&client, images).await {
            Ok(extracted) => extracted,
            Err(e) => return fail(format!("Photo extraction failed: {e}")),
        };
        let output = ExtractRecipeOutput {
            raw_recipe: extracted.raw_recipe,
            method_used: ExtractionMethod::PhotoUpload,
            all_attempts: Vec::new(),
        };
        StepResult {
            step_name: ExtractRecipeStep::NAME.to_string(),
            success: true,
            output: serde_json::to_value(output).expect("ExtractRecipeOutput serializes to JSON"),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: None,
        }
    }
}
