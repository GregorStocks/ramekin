use std::sync::Arc;

use diesel::prelude::*;
use tracing::Instrument;
use uuid::Uuid;

use ramekin_core::ai::{extract_recipe_from_photos, AiClient, CachingAiClient};
use ramekin_core::pipeline::steps::{ExtractRecipeStep, FetchImagesStepMeta, ParseIngredientsStep};
use ramekin_core::{ExtractRecipeOutput, ExtractionMethod, FetchImagesOutput};

use crate::db::DbPool;
use crate::models::{NewScrapeJob, ScrapeJob};
use crate::photos::load_photo_images;
use crate::schema::scrape_jobs;

use super::jobs::{mark_failed, save_step_output, update_status_and_step};
use super::runner::run_scrape_job;
use super::{run_scrape_db, ScrapeError, STATUS_PARSING, STATUS_SCRAPING};

const PHOTO_EXTRACT_STEP: &str = "photo_extract";

/// Create a pending photo import job (no step pre-population yet).
pub async fn create_pending_photo_job(
    pool: &DbPool,
    user_id: Uuid,
) -> Result<ScrapeJob, ScrapeError> {
    run_scrape_db(pool, move |conn| {
        let new_job = NewScrapeJob { user_id, url: None };

        diesel::insert_into(scrape_jobs::table)
            .values(&new_job)
            .get_result::<ScrapeJob>(conn)
            .map_err(|e| ScrapeError::Database(e.to_string()))
    })
    .await
}

/// Spawn a photo import job with proper OpenTelemetry context propagation.
pub fn spawn_photo_import_job(
    pool: Arc<DbPool>,
    job_id: Uuid,
    user_id: Uuid,
    photo_ids: Vec<Uuid>,
) {
    let span = tracing::info_span!(
        "photo_import_job",
        otel.name = "photo_import_job",
        job.id = %job_id,
        job.operation = "photo_import",
        job.status = tracing::field::Empty,
        job.error = tracing::field::Empty,
        photos.count = photo_ids.len(),
    );

    tokio::spawn(
        async move {
            if let Err(e) = run_photo_import_job(pool, job_id, user_id, photo_ids).await {
                let current_span = tracing::Span::current();
                current_span.record("job.status", "failed");
                current_span.record("job.error", tracing::field::display(&e));
                tracing::warn!("Photo import job {} failed: {}", job_id, e);
            }
        }
        .instrument(span),
    );
}

/// Run a photo import job: extract recipe from photos, then run pipeline.
async fn run_photo_import_job(
    pool: Arc<DbPool>,
    job_id: Uuid,
    user_id: Uuid,
    photo_ids: Vec<Uuid>,
) -> Result<(), ScrapeError> {
    match run_photo_import_job_inner(pool.clone(), job_id, user_id, photo_ids).await {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::error!(
                "Photo import job {} failed during extraction: {}",
                job_id,
                e
            );
            mark_failed(&pool, job_id, PHOTO_EXTRACT_STEP, &e.to_string()).await?;
            Err(e)
        }
    }
}

async fn run_photo_import_job_inner(
    pool: Arc<DbPool>,
    job_id: Uuid,
    user_id: Uuid,
    photo_ids: Vec<Uuid>,
) -> Result<(), ScrapeError> {
    // Update status to "scraping" (extraction phase)
    update_status_and_step(&pool, job_id, STATUS_SCRAPING, Some(PHOTO_EXTRACT_STEP)).await?;

    // Step 1: Fetch photo bytes from database
    let images = load_photo_images(&pool, user_id, &photo_ids)
        .await
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    // Step 2: Call vision AI to extract recipe
    let ai_client: Arc<dyn AiClient> = Arc::new(CachingAiClient::from_env()?);
    let extract_result = extract_recipe_from_photos(ai_client.as_ref(), images).await?;

    tracing::info!(
        "Extracted recipe '{}' from photos (cached={})",
        extract_result.raw_recipe.title,
        extract_result.cached
    );

    // Step 3: Pre-populate step outputs and continue pipeline
    let extract_output = ExtractRecipeOutput {
        raw_recipe: extract_result.raw_recipe.clone(),
        method_used: ExtractionMethod::PhotoUpload,
        all_attempts: vec![],
    };
    let extract_json =
        serde_json::to_value(&extract_output).map_err(|e| ScrapeError::Database(e.to_string()))?;

    // fetch_images output (photos already uploaded)
    let images_output = FetchImagesOutput {
        photo_ids,
        failed_urls: vec![],
    };
    let images_json =
        serde_json::to_value(&images_output).map_err(|e| ScrapeError::Database(e.to_string()))?;

    run_scrape_db(&pool, move |conn| {
        save_step_output(conn, job_id, ExtractRecipeStep::NAME, extract_json)?;
        save_step_output(conn, job_id, FetchImagesStepMeta::NAME, images_json)?;

        // Update job to start from parse_ingredients
        let now = chrono::Utc::now();
        diesel::update(scrape_jobs::table.find(job_id))
            .set((
                scrape_jobs::status.eq(STATUS_PARSING),
                scrape_jobs::current_step.eq(Some(ParseIngredientsStep::NAME)),
                scrape_jobs::current_step_started_at.eq(Some(now)),
                scrape_jobs::updated_at.eq(now),
            ))
            .execute(conn)
            .map_err(|e| ScrapeError::Database(e.to_string()))?;

        Ok(())
    })
    .await?;

    // Step 4: Run the rest of the pipeline
    run_scrape_job(pool, job_id).await;
    Ok(())
}
