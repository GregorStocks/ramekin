use chrono::{DateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use ramekin_core::pipeline::steps::{ExtractRecipeStep, FetchImagesStepMeta, ParseIngredientsStep};
use ramekin_core::{ExtractRecipeOutput, ExtractionMethod, FetchHtmlOutput, FetchImagesOutput};
use ramekin_core::{RawRecipe, BUILD_ID};

use crate::db::{DbConn, DbPool};
use crate::models::{NewScrapeJob, NewStepOutput, ScrapeJob};
use crate::schema::{scrape_jobs, step_outputs};

use super::status;
use super::steps::FetchHtmlStep;
use super::{run_scrape_db, ScrapeError, STATUS_COMPLETED, STATUS_FAILED, STATUS_PARSING};

/// Create a new scrape job.
pub async fn create_job(pool: &DbPool, user_id: Uuid, url: &str) -> Result<ScrapeJob, ScrapeError> {
    let url = url.to_string();
    run_scrape_db(pool, move |conn| {
        let new_job = NewScrapeJob {
            user_id,
            url: Some(url.as_str()),
        };

        diesel::insert_into(scrape_jobs::table)
            .values(&new_job)
            .get_result::<ScrapeJob>(conn)
            .map_err(|e| ScrapeError::Database(e.to_string()))
    })
    .await
}

/// Create a rescrape job for an existing recipe.
/// This pre-populates recipe_id so save_recipe knows to update instead of create.
pub async fn create_rescrape_job(
    pool: &DbPool,
    user_id: Uuid,
    recipe_id: Uuid,
    expected_version_id: Uuid,
    url: &str,
) -> Result<ScrapeJob, ScrapeError> {
    let url = url.to_string();
    run_scrape_db(pool, move |conn| {
        diesel::insert_into(scrape_jobs::table)
            .values((
                scrape_jobs::user_id.eq(user_id),
                scrape_jobs::url.eq(url.as_str()),
                scrape_jobs::recipe_id.eq(Some(recipe_id)),
                scrape_jobs::expected_version_id.eq(Some(expected_version_id)),
            ))
            .get_result::<ScrapeJob>(conn)
            .map_err(|e| ScrapeError::Database(e.to_string()))
    })
    .await
}

/// Create a photo-only rescrape job. The pipeline runs normally but the save
/// step only updates `photo_ids`, carrying forward every other field from the
/// current version.
pub async fn create_photo_rescrape_job(
    pool: &DbPool,
    user_id: Uuid,
    recipe_id: Uuid,
    expected_version_id: Uuid,
    url: &str,
) -> Result<ScrapeJob, ScrapeError> {
    let url = url.to_string();
    run_scrape_db(pool, move |conn| {
        diesel::insert_into(scrape_jobs::table)
            .values((
                scrape_jobs::user_id.eq(user_id),
                scrape_jobs::url.eq(url.as_str()),
                scrape_jobs::recipe_id.eq(Some(recipe_id)),
                scrape_jobs::expected_version_id.eq(Some(expected_version_id)),
                scrape_jobs::photo_only.eq(true),
            ))
            .get_result::<ScrapeJob>(conn)
            .map_err(|e| ScrapeError::Database(e.to_string()))
    })
    .await
}

/// Create a new scrape job with pre-existing HTML (for bookmarklet capture).
/// This creates the job, stores the HTML as the fetch_html output,
/// and sets the job to start from the extract_recipe step.
pub async fn create_job_with_html(
    pool: &DbPool,
    user_id: Uuid,
    url: &str,
    html: String,
) -> Result<ScrapeJob, ScrapeError> {
    let url = url.to_string();
    run_scrape_db(pool, move |conn| {
        // Create the job
        let new_job = NewScrapeJob {
            user_id,
            url: Some(url.as_str()),
        };
        let job: ScrapeJob = diesel::insert_into(scrape_jobs::table)
            .values(&new_job)
            .get_result(conn)
            .map_err(|e| ScrapeError::Database(e.to_string()))?;

        // Store the HTML as the fetch_html step output
        let fetch_output = FetchHtmlOutput { html };
        let output_json = serde_json::to_value(&fetch_output)
            .map_err(|e| ScrapeError::Database(e.to_string()))?;
        save_step_output(conn, job.id, FetchHtmlStep::NAME, output_json)?;

        // Update the job to start from parsing (skip fetch step)
        let now = Utc::now();
        diesel::update(scrape_jobs::table.find(job.id))
            .set((
                scrape_jobs::status.eq(STATUS_PARSING),
                scrape_jobs::current_step.eq(Some(ExtractRecipeStep::NAME)),
                scrape_jobs::current_step_started_at.eq(Some(now)),
                scrape_jobs::updated_at.eq(now),
            ))
            .execute(conn)
            .map_err(|e| ScrapeError::Database(e.to_string()))?;

        // Return the updated job
        get_job_conn(conn, job.id)
    })
    .await
}

/// Create an import job with pre-populated extract_recipe and fetch_images outputs.
/// This allows imports to skip the fetch and extract steps and start directly at parse_ingredients.
pub async fn create_import_job(
    pool: &DbPool,
    user_id: Uuid,
    source_url: Option<&str>,
    raw_recipe: RawRecipe,
    extraction_method: ExtractionMethod,
    photo_ids: Vec<Uuid>,
) -> Result<ScrapeJob, ScrapeError> {
    let source_url = source_url.map(str::to_string);
    run_scrape_db(pool, move |conn| {
        // Create the job (url is optional for imports)
        let new_job = NewScrapeJob {
            user_id,
            url: source_url.as_deref(),
        };
        let job: ScrapeJob = diesel::insert_into(scrape_jobs::table)
            .values(&new_job)
            .get_result(conn)
            .map_err(|e| ScrapeError::Database(e.to_string()))?;

        // Store the extract_recipe step output
        let extract_output = ExtractRecipeOutput {
            raw_recipe,
            method_used: extraction_method,
            all_attempts: vec![],
        };
        let extract_json = serde_json::to_value(&extract_output)
            .map_err(|e| ScrapeError::Database(e.to_string()))?;
        save_step_output(conn, job.id, ExtractRecipeStep::NAME, extract_json)?;

        // Store the fetch_images step output (photos already uploaded)
        let images_output = FetchImagesOutput {
            photo_ids,
            failed_urls: vec![],
        };
        let images_json = serde_json::to_value(&images_output)
            .map_err(|e| ScrapeError::Database(e.to_string()))?;
        save_step_output(conn, job.id, FetchImagesStepMeta::NAME, images_json)?;

        // Update the job to start from parse_ingredients (skip fetch_html, extract_recipe, fetch_images)
        let now = Utc::now();
        diesel::update(scrape_jobs::table.find(job.id))
            .set((
                scrape_jobs::status.eq(STATUS_PARSING),
                scrape_jobs::current_step.eq(Some(ParseIngredientsStep::NAME)),
                scrape_jobs::current_step_started_at.eq(Some(now)),
                scrape_jobs::updated_at.eq(now),
            ))
            .execute(conn)
            .map_err(|e| ScrapeError::Database(e.to_string()))?;

        // Return the updated job
        get_job_conn(conn, job.id)
    })
    .await
}

/// Get a scrape job by ID.
pub async fn get_job(pool: &DbPool, job_id: Uuid) -> Result<ScrapeJob, ScrapeError> {
    run_scrape_db(pool, move |conn| get_job_conn(conn, job_id)).await
}

/// Get a scrape job by ID using an already checked-out connection.
fn get_job_conn(conn: &mut DbConn, job_id: Uuid) -> Result<ScrapeJob, ScrapeError> {
    scrape_jobs::table
        .find(job_id)
        .first::<ScrapeJob>(conn)
        .optional()
        .map_err(|e| ScrapeError::Database(e.to_string()))?
        .ok_or(ScrapeError::JobNotFound)
}

/// Update job status and current_step.
///
/// Also sets `current_step_started_at` to `NOW()` when a step is provided, or
/// clears it when the step is cleared. The frontend uses this timestamp to
/// show how long the currently running step has been executing.
pub(super) async fn update_status_and_step(
    pool: &DbPool,
    job_id: Uuid,
    status: &str,
    current_step: Option<&str>,
) -> Result<(), ScrapeError> {
    let status = status.to_string();
    let current_step = current_step.map(str::to_string);
    run_scrape_db(pool, move |conn| {
        let now = Utc::now();
        match current_step {
            Some(step) => {
                diesel::update(scrape_jobs::table.find(job_id))
                    .set((
                        scrape_jobs::status.eq(status.as_str()),
                        scrape_jobs::current_step.eq(Some(step.as_str())),
                        scrape_jobs::current_step_started_at.eq(Some(now)),
                        scrape_jobs::updated_at.eq(now),
                    ))
                    .execute(conn)
                    .map_err(|e| ScrapeError::Database(e.to_string()))?;
            }
            None => {
                diesel::update(scrape_jobs::table.find(job_id))
                    .set((
                        scrape_jobs::status.eq(status.as_str()),
                        scrape_jobs::current_step.eq::<Option<String>>(None),
                        scrape_jobs::current_step_started_at.eq::<Option<DateTime<Utc>>>(None),
                        scrape_jobs::updated_at.eq(now),
                    ))
                    .execute(conn)
                    .map_err(|e| ScrapeError::Database(e.to_string()))?;
            }
        }

        Ok(())
    })
    .await
}

/// Save a step output to the database (append-only).
pub(super) fn save_step_output(
    conn: &mut DbConn,
    job_id: Uuid,
    step_name: &str,
    output: serde_json::Value,
) -> Result<(), ScrapeError> {
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
        .execute(conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(())
}

/// Mark job as failed.
///
/// `step_name` is the real pipeline step name that failed (e.g. `"fetch_html"`,
/// `"extract_recipe"`, `"photo_extract"`), not a job status string. The status
/// page and retry logic both key off this value.
pub(super) async fn mark_failed(
    pool: &DbPool,
    job_id: Uuid,
    step_name: &str,
    error: &str,
) -> Result<(), ScrapeError> {
    let step_name = step_name.to_string();
    let error = error.to_string();
    run_scrape_db(pool, move |conn| {
        // Leave current_step_started_at set so the status API can show when the failed step started.
        diesel::update(scrape_jobs::table.find(job_id))
            .set((
                scrape_jobs::status.eq(STATUS_FAILED),
                scrape_jobs::failed_at_step.eq(Some(step_name.as_str())),
                scrape_jobs::error_message.eq(Some(error.as_str())),
                scrape_jobs::updated_at.eq(Utc::now()),
            ))
            .execute(conn)
            .map_err(|e| ScrapeError::Database(e.to_string()))?;

        Ok(())
    })
    .await
}

/// Mark job as completed with recipe ID.
pub(super) async fn mark_completed(
    pool: &DbPool,
    job_id: Uuid,
    recipe_id: Uuid,
) -> Result<(), ScrapeError> {
    run_scrape_db(pool, move |conn| {
        diesel::update(scrape_jobs::table.find(job_id))
            .set((
                scrape_jobs::status.eq(STATUS_COMPLETED),
                scrape_jobs::recipe_id.eq(Some(recipe_id)),
                scrape_jobs::current_step.eq::<Option<String>>(None),
                scrape_jobs::current_step_started_at.eq::<Option<DateTime<Utc>>>(None),
                scrape_jobs::updated_at.eq(Utc::now()),
            ))
            .execute(conn)
            .map_err(|e| ScrapeError::Database(e.to_string()))?;

        Ok(())
    })
    .await
}
