mod output_store;
pub mod steps;

use crate::db::DbPool;
use crate::models::{NewScrapeJob, NewStepOutput, ScrapeJob, StepOutput};
use crate::schema::{scrape_jobs, step_outputs};
use chrono::Utc;
use diesel::prelude::*;
use ramekin_core::pipeline::steps::{ExtractRecipeStep, FetchImagesStepMeta, SaveRecipeStepMeta};
use ramekin_core::pipeline::{run_pipeline, StepOutputStore, StepRegistry};
use ramekin_core::{FetchHtmlOutput, BUILD_ID};
use std::env;
use std::sync::Arc;
use thiserror::Error;
use tracing::Instrument;
use uuid::Uuid;

use output_store::DbOutputStore;
use steps::{EnrichStep, FetchHtmlStep, FetchImagesStep, SaveRecipeStep};

#[derive(Error, Debug)]
pub enum ScrapeError {
    #[error("Fetch error: {0}")]
    Fetch(#[from] ramekin_core::FetchError),

    #[error("Parse error: {0}")]
    Parse(#[from] ramekin_core::ExtractError),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Job not found")]
    JobNotFound,

    #[error("Invalid job state: {0}")]
    InvalidState(String),

    #[error("URL host not allowed: {0}")]
    HostNotAllowed(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Max retries exceeded")]
    MaxRetriesExceeded,
}

/// Job statuses
pub const STATUS_PENDING: &str = "pending";
pub const STATUS_SCRAPING: &str = "scraping";
pub const STATUS_PARSING: &str = "parsing";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_FAILED: &str = "failed";

/// Maximum retries before hard fail
const MAX_RETRIES: i32 = 5;

/// Check if a URL's host is allowed for scraping.
/// If SCRAPE_ALLOWED_HOSTS is set, only those hosts are allowed.
/// If not set, all hosts are allowed (production mode).
pub fn is_host_allowed(url: &str) -> Result<(), ScrapeError> {
    let parsed = reqwest::Url::parse(url).map_err(|e| ScrapeError::InvalidUrl(e.to_string()))?;

    let host = parsed
        .host_str()
        .ok_or_else(|| ScrapeError::InvalidUrl("No host in URL".to_string()))?;

    // Check for allowed hosts (used in tests)
    if let Ok(allowed) = env::var("SCRAPE_ALLOWED_HOSTS") {
        let allowed_hosts: Vec<&str> = allowed.split(',').map(|s| s.trim()).collect();
        // Include port if present
        let host_with_port = if let Some(port) = parsed.port() {
            format!("{}:{}", host, port)
        } else {
            host.to_string()
        };

        if !allowed_hosts
            .iter()
            .any(|&h| h == host_with_port || h == host)
        {
            return Err(ScrapeError::HostNotAllowed(host_with_port));
        }
    }

    Ok(())
}

/// Build a step registry for server-side pipeline execution.
///
/// This creates all step implementations with the necessary resources (DB pool, user ID).
pub fn build_registry(pool: Arc<DbPool>, user_id: Uuid) -> StepRegistry {
    let mut registry = StepRegistry::new();
    registry.register(Box::new(FetchHtmlStep));
    registry.register(Box::new(ExtractRecipeStep));
    registry.register(Box::new(FetchImagesStep::new(pool.clone(), user_id)));
    registry.register(Box::new(SaveRecipeStep::new(pool, user_id)));
    registry.register(Box::new(EnrichStep));
    registry
}

/// Create a new scrape job.
pub fn create_job(pool: &DbPool, user_id: Uuid, url: &str) -> Result<ScrapeJob, ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    let new_job = NewScrapeJob { user_id, url };

    diesel::insert_into(scrape_jobs::table)
        .values(&new_job)
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
    let new_job = NewScrapeJob { user_id, url };
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
    diesel::update(scrape_jobs::table.find(job.id))
        .set((
            scrape_jobs::status.eq(STATUS_PARSING),
            scrape_jobs::current_step.eq(Some(ExtractRecipeStep::NAME)),
            scrape_jobs::updated_at.eq(Utc::now()),
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
fn update_status_and_step(
    pool: &DbPool,
    job_id: Uuid,
    status: &str,
    current_step: Option<&str>,
) -> Result<(), ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    diesel::update(scrape_jobs::table.find(job_id))
        .set((
            scrape_jobs::status.eq(status),
            scrape_jobs::current_step.eq(current_step),
            scrape_jobs::updated_at.eq(Utc::now()),
        ))
        .execute(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(())
}

/// Save a step output to the database (append-only).
fn save_step_output(
    pool: &DbPool,
    job_id: Uuid,
    step_name: &str,
    output: serde_json::Value,
) -> Result<(), ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    let new_output = NewStepOutput {
        scrape_job_id: job_id,
        step_name: step_name.to_string(),
        build_id: BUILD_ID.to_string(),
        output,
    };

    diesel::insert_into(step_outputs::table)
        .values(&new_output)
        .execute(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(())
}

/// Get the most recent step output for a job by step name.
fn get_latest_step_output(
    pool: &DbPool,
    job_id: Uuid,
    step_name: &str,
) -> Result<Option<StepOutput>, ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    step_outputs::table
        .filter(step_outputs::scrape_job_id.eq(job_id))
        .filter(step_outputs::step_name.eq(step_name))
        .order(step_outputs::created_at.desc())
        .first::<StepOutput>(&mut conn)
        .optional()
        .map_err(|e| ScrapeError::Database(e.to_string()))
}

/// Mark job as failed.
fn mark_failed(pool: &DbPool, job_id: Uuid, step: &str, error: &str) -> Result<(), ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    diesel::update(scrape_jobs::table.find(job_id))
        .set((
            scrape_jobs::status.eq(STATUS_FAILED),
            scrape_jobs::failed_at_step.eq(Some(step)),
            scrape_jobs::error_message.eq(Some(error)),
            scrape_jobs::updated_at.eq(Utc::now()),
        ))
        .execute(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(())
}

/// Mark job as completed with recipe ID.
fn mark_completed(pool: &DbPool, job_id: Uuid, recipe_id: Uuid) -> Result<(), ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    diesel::update(scrape_jobs::table.find(job_id))
        .set((
            scrape_jobs::status.eq(STATUS_COMPLETED),
            scrape_jobs::recipe_id.eq(Some(recipe_id)),
            scrape_jobs::current_step.eq::<Option<String>>(None),
            scrape_jobs::updated_at.eq(Utc::now()),
        ))
        .execute(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(())
}

/// Spawn a scrape job with proper OpenTelemetry context propagation.
///
/// This creates a span that:
/// - Links to the current (HTTP request) span as parent
/// - Contains job metadata (job_id, url, operation type)
/// - Wraps the entire job execution
pub fn spawn_scrape_job(pool: Arc<DbPool>, job_id: Uuid, url: &str, operation: &str) {
    let domain = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(String::from))
        .unwrap_or_else(|| "unknown".to_string());

    let span = tracing::info_span!(
        "scrape_job",
        otel.name = %format!("scrape_job {}", operation),
        job.id = %job_id,
        job.operation = %operation,
        url.full = %url,
        url.domain = %domain,
        job.status = tracing::field::Empty,
        job.error = tracing::field::Empty,
    );

    tokio::spawn(
        async move {
            run_scrape_job(pool, job_id).await;
        }
        .instrument(span),
    );
}

/// Run the scrape job state machine.
/// This processes the job through its states: pending -> scraping -> parsing -> completed
pub async fn run_scrape_job(pool: Arc<DbPool>, job_id: Uuid) {
    let result = run_scrape_job_inner(pool, job_id).await;
    let current_span = tracing::Span::current();

    match &result {
        Ok(()) => {
            current_span.record("job.status", "completed");
        }
        Err(e) => {
            current_span.record("job.status", "failed");
            current_span.record("job.error", tracing::field::display(e));
            tracing::warn!("Scrape job {} failed: {}", job_id, e);
        }
    }
}

async fn run_scrape_job_inner(pool: Arc<DbPool>, job_id: Uuid) -> Result<(), ScrapeError> {
    let job = get_job(&pool, job_id)?;

    // Terminal states - nothing to do
    if job.status == STATUS_COMPLETED || job.status == STATUS_FAILED {
        return Ok(());
    }

    // Determine starting step
    let first_step = job.current_step.as_deref().unwrap_or(FetchHtmlStep::NAME);

    // Update status to scraping/parsing based on first step
    let status = if first_step == FetchHtmlStep::NAME {
        STATUS_SCRAPING
    } else {
        STATUS_PARSING
    };
    update_status_and_step(&pool, job_id, status, Some(first_step))?;

    tracing::info!(
        "Job {} starting pipeline from step '{}' in status '{}'",
        job_id,
        first_step,
        status
    );

    // Build the step registry and output store
    let registry = build_registry(pool.clone(), job.user_id);
    let mut store = DbOutputStore::new(&pool, job_id);

    // Run the generic pipeline
    let results = run_pipeline(first_step, &job.url, &mut store, &registry).await;

    // Determine which step failed by tracking the step chain
    // fetch_html is "scraping", all other steps are "parsing"
    let mut current_step = first_step;
    let mut failed_at_status = status;
    for result in &results {
        // Determine the phase for this step
        failed_at_status = if current_step == FetchHtmlStep::NAME {
            STATUS_SCRAPING
        } else {
            STATUS_PARSING
        };

        if result.success {
            tracing::debug!(
                "Step '{}' completed successfully in {}ms",
                current_step,
                result.duration_ms
            );
            // Move to next step
            if let Some(ref next) = result.next_step {
                current_step = next.as_str();
            }
        } else if let Some(ref error) = result.error {
            tracing::debug!("Step '{}' failed: {}", current_step, error);
            // Don't update current_step - we want to record where it failed
        }
    }

    // Determine final outcome from results
    let last_result = results.last();

    match last_result {
        Some(result) => {
            // Check if we completed successfully (save_recipe succeeded)
            let recipe_id = store
                .get_output(SaveRecipeStepMeta::NAME)
                .and_then(|o| {
                    o.get("recipe_id")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                })
                .and_then(|s| Uuid::parse_str(&s).ok());

            if let Some(id) = recipe_id {
                // Pipeline completed through save_recipe
                tracing::info!("Job {} completed successfully, recipe_id={}", job_id, id);
                mark_completed(&pool, job_id, id)?;
            } else if !result.success && result.next_step.is_none() {
                // Pipeline failed (step returned failure with no next step)
                let error = result.error.as_deref().unwrap_or("Unknown error");
                tracing::warn!("Job {} failed at '{}': {}", job_id, failed_at_status, error);
                mark_failed(&pool, job_id, failed_at_status, error)?;
            } else {
                // Pipeline ended without a recipe (shouldn't happen in normal flow)
                tracing::warn!("Job {} ended without recipe", job_id);
                mark_failed(
                    &pool,
                    job_id,
                    failed_at_status,
                    "Pipeline ended without creating recipe",
                )?;
            }
        }
        None => {
            // No results at all (shouldn't happen)
            tracing::warn!("Job {} produced no results", job_id);
            mark_failed(&pool, job_id, failed_at_status, "No pipeline results")?;
        }
    }

    Ok(())
}

/// Reset a failed job for retry.
/// Returns the status to resume from.
pub fn retry_job(pool: &DbPool, job_id: Uuid) -> Result<String, ScrapeError> {
    let job = get_job(pool, job_id)?;

    if job.status != STATUS_FAILED {
        return Err(ScrapeError::InvalidState(format!(
            "Cannot retry job in status: {}",
            job.status
        )));
    }

    // Check retry count
    if job.retry_count >= MAX_RETRIES {
        return Err(ScrapeError::MaxRetriesExceeded);
    }

    // Determine where to resume based on failed_at_step and available outputs
    let has_fetch_output = get_latest_step_output(pool, job_id, FetchHtmlStep::NAME)?.is_some();
    let has_extract_output =
        get_latest_step_output(pool, job_id, ExtractRecipeStep::NAME)?.is_some();
    let has_fetch_images_output =
        get_latest_step_output(pool, job_id, FetchImagesStepMeta::NAME)?.is_some();

    let (resume_status, resume_step) = match job.failed_at_step.as_deref() {
        Some(STATUS_SCRAPING) => {
            // Failed during fetch - restart from fetch
            (STATUS_SCRAPING, FetchHtmlStep::NAME)
        }
        Some(STATUS_PARSING) => {
            if has_fetch_images_output {
                // Have fetch_images output, try save again
                (STATUS_PARSING, SaveRecipeStepMeta::NAME)
            } else if has_extract_output {
                // Have extract output, try fetch_images
                (STATUS_PARSING, FetchImagesStepMeta::NAME)
            } else if has_fetch_output {
                // Have fetch output, try extract again
                (STATUS_PARSING, ExtractRecipeStep::NAME)
            } else {
                // No outputs, start from beginning
                (STATUS_SCRAPING, FetchHtmlStep::NAME)
            }
        }
        _ => {
            // Unknown failure point, start from beginning
            (STATUS_PENDING, FetchHtmlStep::NAME)
        }
    };

    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    diesel::update(scrape_jobs::table.find(job_id))
        .set((
            scrape_jobs::status.eq(resume_status),
            scrape_jobs::current_step.eq(Some(resume_step)),
            scrape_jobs::failed_at_step.eq::<Option<String>>(None),
            scrape_jobs::error_message.eq::<Option<String>>(None),
            scrape_jobs::retry_count.eq(job.retry_count + 1),
            scrape_jobs::updated_at.eq(Utc::now()),
        ))
        .execute(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(resume_status.to_string())
}
