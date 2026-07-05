use chrono::{DateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use ramekin_core::pipeline::steps::{ExtractRecipeStep, FetchImagesStepMeta, ParseIngredientsStep};
use ramekin_core::{ExtractRecipeOutput, ExtractionMethod, FetchHtmlOutput, FetchImagesOutput};
use ramekin_core::{RawRecipe, BUILD_ID};

use crate::db::DbPool;
use crate::models::{NewScrapeJob, NewStepOutput, ScrapeJob};
use crate::schema::{scrape_jobs, step_outputs};

use super::status;
use super::steps::FetchHtmlStep;
use super::{ScrapeError, STATUS_COMPLETED, STATUS_FAILED, STATUS_PARSING};

/// Create a new scrape job.
pub fn create_job(pool: &DbPool, user_id: Uuid, url: &str) -> Result<ScrapeJob, ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    let new_job = NewScrapeJob {
        user_id,
        url: Some(url),
    };

    diesel::insert_into(scrape_jobs::table)
        .values(&new_job)
        .get_result::<ScrapeJob>(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))
}

/// Create a rescrape job for an existing recipe.
/// This pre-populates recipe_id so save_recipe knows to update instead of create.
pub fn create_rescrape_job(
    pool: &DbPool,
    user_id: Uuid,
    recipe_id: Uuid,
    url: &str,
) -> Result<ScrapeJob, ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    diesel::insert_into(scrape_jobs::table)
        .values((
            scrape_jobs::user_id.eq(user_id),
            scrape_jobs::url.eq(url),
            scrape_jobs::recipe_id.eq(Some(recipe_id)),
        ))
        .get_result::<ScrapeJob>(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))
}

/// Create a photo-only rescrape job. The pipeline runs normally but the save
/// step only updates `photo_ids`, carrying forward every other field from the
/// current version.
pub fn create_photo_rescrape_job(
    pool: &DbPool,
    user_id: Uuid,
    recipe_id: Uuid,
    url: &str,
) -> Result<ScrapeJob, ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    diesel::insert_into(scrape_jobs::table)
        .values((
            scrape_jobs::user_id.eq(user_id),
            scrape_jobs::url.eq(url),
            scrape_jobs::recipe_id.eq(Some(recipe_id)),
            scrape_jobs::photo_only.eq(true),
        ))
        .get_result::<ScrapeJob>(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))
}

/// Create a new scrape job with pre-existing HTML (for bookmarklet capture).
/// This creates the job, stores the HTML as the fetch_html output,
/// and sets the job to start from the extract_recipe step.
pub fn create_job_with_html(
    pool: &DbPool,
    user_id: Uuid,
    url: &str,
    html: &str,
) -> Result<ScrapeJob, ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    // Create the job
    let new_job = NewScrapeJob {
        user_id,
        url: Some(url),
    };
    let job: ScrapeJob = diesel::insert_into(scrape_jobs::table)
        .values(&new_job)
        .get_result(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    // Store the HTML as the fetch_html step output
    let fetch_output = FetchHtmlOutput {
        html: html.to_string(),
    };
    let output_json =
        serde_json::to_value(&fetch_output).map_err(|e| ScrapeError::Database(e.to_string()))?;
    save_step_output(pool, job.id, FetchHtmlStep::NAME, output_json)?;

    // Update the job to start from parsing (skip fetch step)
    let now = Utc::now();
    diesel::update(scrape_jobs::table.find(job.id))
        .set((
            scrape_jobs::status.eq(STATUS_PARSING),
            scrape_jobs::current_step.eq(Some(ExtractRecipeStep::NAME)),
            scrape_jobs::current_step_started_at.eq(Some(now)),
            scrape_jobs::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    // Return the updated job
    get_job(pool, job.id)
}

/// Create an import job with pre-populated extract_recipe and fetch_images outputs.
/// This allows imports to skip the fetch and extract steps and start directly at parse_ingredients.
pub fn create_import_job(
    pool: &DbPool,
    user_id: Uuid,
    source_url: Option<&str>,
    raw_recipe: &RawRecipe,
    extraction_method: ExtractionMethod,
    photo_ids: Vec<Uuid>,
) -> Result<ScrapeJob, ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    // Create the job (url is optional for imports)
    let new_job = NewScrapeJob {
        user_id,
        url: source_url,
    };
    let job: ScrapeJob = diesel::insert_into(scrape_jobs::table)
        .values(&new_job)
        .get_result(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    // Store the extract_recipe step output
    let extract_output = ExtractRecipeOutput {
        raw_recipe: raw_recipe.clone(),
        method_used: extraction_method,
        all_attempts: vec![],
    };
    let extract_json =
        serde_json::to_value(&extract_output).map_err(|e| ScrapeError::Database(e.to_string()))?;
    save_step_output(pool, job.id, ExtractRecipeStep::NAME, extract_json)?;

    // Store the fetch_images step output (photos already uploaded)
    let images_output = FetchImagesOutput {
        photo_ids,
        failed_urls: vec![],
    };
    let images_json =
        serde_json::to_value(&images_output).map_err(|e| ScrapeError::Database(e.to_string()))?;
    save_step_output(pool, job.id, FetchImagesStepMeta::NAME, images_json)?;

    // Update the job to start from parse_ingredients (skip fetch_html, extract_recipe, fetch_images)
    let now = Utc::now();
    diesel::update(scrape_jobs::table.find(job.id))
        .set((
            scrape_jobs::status.eq(STATUS_PARSING),
            scrape_jobs::current_step.eq(Some(ParseIngredientsStep::NAME)),
            scrape_jobs::current_step_started_at.eq(Some(now)),
            scrape_jobs::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    // Return the updated job
    get_job(pool, job.id)
}

/// Get a scrape job by ID.
pub fn get_job(pool: &DbPool, job_id: Uuid) -> Result<ScrapeJob, ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    scrape_jobs::table
        .find(job_id)
        .first::<ScrapeJob>(&mut conn)
        .optional()
        .map_err(|e| ScrapeError::Database(e.to_string()))?
        .ok_or(ScrapeError::JobNotFound)
}

/// Update job status and current_step.
///
/// Also sets `current_step_started_at` to `NOW()` when a step is provided, or
/// clears it when the step is cleared. The frontend uses this timestamp to
/// show how long the currently running step has been executing.
pub(super) fn update_status_and_step(
    pool: &DbPool,
    job_id: Uuid,
    status: &str,
    current_step: Option<&str>,
) -> Result<(), ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    let now = Utc::now();
    match current_step {
        Some(step) => {
            diesel::update(scrape_jobs::table.find(job_id))
                .set((
                    scrape_jobs::status.eq(status),
                    scrape_jobs::current_step.eq(Some(step)),
                    scrape_jobs::current_step_started_at.eq(Some(now)),
                    scrape_jobs::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .map_err(|e| ScrapeError::Database(e.to_string()))?;
        }
        None => {
            diesel::update(scrape_jobs::table.find(job_id))
                .set((
                    scrape_jobs::status.eq(status),
                    scrape_jobs::current_step.eq::<Option<String>>(None),
                    scrape_jobs::current_step_started_at.eq::<Option<DateTime<Utc>>>(None),
                    scrape_jobs::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .map_err(|e| ScrapeError::Database(e.to_string()))?;
        }
    }

    Ok(())
}

/// Save a step output to the database (append-only).
pub(super) fn save_step_output(
    pool: &DbPool,
    job_id: Uuid,
    step_name: &str,
    output: serde_json::Value,
) -> Result<(), ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    let summary = status::step_summary(step_name, &output);
    let new_output = NewStepOutput {
        scrape_job_id: job_id,
        step_name: step_name.to_string(),
        build_id: BUILD_ID.to_string(),
        output,
        // Pre-populated outputs (HTML capture / photo import / recipe import)
        // don't run the step, so there's no meaningful duration to record.
        duration_ms: None,
        summary,
        // Pre-populated outputs are always successful — the step didn't run
        // so there's no failure to record.
        success: true,
        error: None,
    };

    diesel::insert_into(step_outputs::table)
        .values(&new_output)
        .execute(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(())
}

/// Mark job as failed.
///
/// `step_name` is the real pipeline step name that failed (e.g. `"fetch_html"`,
/// `"extract_recipe"`, `"photo_extract"`), not a job status string. The status
/// page and retry logic both key off this value.
pub(super) fn mark_failed(
    pool: &DbPool,
    job_id: Uuid,
    step_name: &str,
    error: &str,
) -> Result<(), ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    // Leave current_step_started_at set so the status API can show when the failed step started.
    diesel::update(scrape_jobs::table.find(job_id))
        .set((
            scrape_jobs::status.eq(STATUS_FAILED),
            scrape_jobs::failed_at_step.eq(Some(step_name)),
            scrape_jobs::error_message.eq(Some(error)),
            scrape_jobs::updated_at.eq(Utc::now()),
        ))
        .execute(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(())
}

/// Mark job as completed with recipe ID.
pub(super) fn mark_completed(
    pool: &DbPool,
    job_id: Uuid,
    recipe_id: Uuid,
) -> Result<(), ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    diesel::update(scrape_jobs::table.find(job_id))
        .set((
            scrape_jobs::status.eq(STATUS_COMPLETED),
            scrape_jobs::recipe_id.eq(Some(recipe_id)),
            scrape_jobs::current_step.eq::<Option<String>>(None),
            scrape_jobs::current_step_started_at.eq::<Option<DateTime<Utc>>>(None),
            scrape_jobs::updated_at.eq(Utc::now()),
        ))
        .execute(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(())
}
