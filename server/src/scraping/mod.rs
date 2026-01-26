mod output_store;
pub mod steps;

use crate::db::DbPool;
use crate::models::{NewScrapeJob, NewStepOutput, ScrapeJob, StepOutput};
use crate::schema::{scrape_jobs, step_outputs};
use chrono::Utc;
use diesel::prelude::*;
use ramekin_core::pipeline::steps::{ExtractRecipeStep, FetchImagesStepMeta, SaveRecipeStepMeta};
use ramekin_core::pipeline::{PipelineStep, StepContext, StepOutputStore, StepRegistry};
use ramekin_core::{FetchHtmlOutput, BUILD_ID};
use std::env;
use std::sync::Arc;
use thiserror::Error;
use tracing::Instrument;
use uuid::Uuid;

use output_store::DbOutputStore;
use steps::{
    EnrichAutoTagStep, EnrichGeneratePhotoStep, EnrichNormalizeIngredientsStep, FetchHtmlStep,
    FetchImagesStep, SaveRecipeStep,
};

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
    registry.register(Box::new(EnrichNormalizeIngredientsStep));
    registry.register(Box::new(EnrichAutoTagStep));
    registry.register(Box::new(EnrichGeneratePhotoStep));
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

    tracing::info!(
        "Job {} starting pipeline from step '{}'",
        job_id,
        first_step
    );

    // Build the step registry and output store
    let registry = build_registry(pool.clone(), job.user_id);
    let mut store = DbOutputStore::new(&pool, job_id);

    // Run pipeline with status updates and OpenTelemetry instrumentation
    let mut current_step_name: Option<String> = Some(first_step.to_string());
    let mut failed_at_status = STATUS_SCRAPING;
    let mut last_error: Option<String> = None;

    while let Some(step_name) = current_step_name.take() {
        let step = match registry.get(&step_name) {
            Some(s) => s,
            None => {
                tracing::warn!("Unknown step '{}', stopping pipeline", step_name);
                break;
            }
        };

        // Determine status for this step: fetch_html is "scraping", all others are "parsing"
        let step_status = if step_name == FetchHtmlStep::NAME {
            STATUS_SCRAPING
        } else {
            STATUS_PARSING
        };
        failed_at_status = step_status;

        // Update job status and current_step before executing
        update_status_and_step(&pool, job_id, step_status, Some(&step_name))?;

        // Execute step with OpenTelemetry span
        let result = execute_step_with_tracing(step, &job.url, &store, &step_name).await;

        // Save output (for both success and failure - useful for debugging)
        if let Err(e) = store.save_output(&step_name, &result.output) {
            tracing::warn!("Failed to save output for step {}: {}", step_name, e);
        }

        let meta = step.metadata();
        let should_continue = result.success || meta.continues_on_failure;

        if result.success {
            tracing::debug!(
                "Step '{}' completed successfully in {}ms",
                step_name,
                result.duration_ms
            );
        } else {
            last_error = result.error.clone();
            tracing::debug!(
                "Step '{}' failed: {}",
                step_name,
                last_error.as_deref().unwrap_or("unknown error")
            );
        }

        if !should_continue {
            break;
        }

        current_step_name = result.next_step;
    }

    // Determine final outcome
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
    } else if let Some(error) = last_error {
        // Pipeline failed
        tracing::warn!("Job {} failed at '{}': {}", job_id, failed_at_status, error);
        mark_failed(&pool, job_id, failed_at_status, &error)?;
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

    Ok(())
}

/// Execute a pipeline step with OpenTelemetry tracing.
///
/// Creates a span for the step with relevant attributes and records
/// step-specific data after execution.
async fn execute_step_with_tracing(
    step: &dyn PipelineStep,
    url: &str,
    store: &dyn StepOutputStore,
    step_name: &str,
) -> ramekin_core::pipeline::StepResult {
    use ramekin_core::pipeline::StepResult;

    let span = tracing::info_span!(
        "scrape_step",
        otel.name = %format!("{}", step_name),
        step.name = %step_name,
        step.success = tracing::field::Empty,
        step.duration_ms = tracing::field::Empty,
        step.error = tracing::field::Empty,
        // Step-specific fields (recorded after execution)
        http.url = tracing::field::Empty,
        http.response_content_length = tracing::field::Empty,
        recipe.title = tracing::field::Empty,
        recipe.id = tracing::field::Empty,
        images.requested = tracing::field::Empty,
        images.success = tracing::field::Empty,
        images.failed = tracing::field::Empty,
    );

    let ctx = StepContext {
        url,
        outputs: store,
    };

    let result: StepResult = async {
        let result = step.execute(&ctx).await;

        // Record common fields
        let current_span = tracing::Span::current();
        current_span.record("step.success", result.success);
        current_span.record("step.duration_ms", result.duration_ms);
        if let Some(ref error) = result.error {
            current_span.record("step.error", error.as_str());
        }

        // Record step-specific fields based on output
        match step_name {
            "fetch_html" => {
                current_span.record("http.url", url);
                if let Some(html) = result.output.get("html").and_then(|v| v.as_str()) {
                    current_span.record("http.response_content_length", html.len());
                }
            }
            "extract_recipe" => {
                if let Some(title) = result
                    .output
                    .get("raw_recipe")
                    .and_then(|r| r.get("title"))
                    .and_then(|t| t.as_str())
                {
                    current_span.record("recipe.title", title);
                }
            }
            "fetch_images" => {
                // Get requested count from extract_recipe output
                if let Some(extract_output) = store.get_output("extract_recipe") {
                    if let Some(urls) = extract_output
                        .get("raw_recipe")
                        .and_then(|r| r.get("image_urls"))
                        .and_then(|u| u.as_array())
                    {
                        current_span.record("images.requested", urls.len());
                    }
                }
                if let Some(photo_ids) = result.output.get("photo_ids").and_then(|v| v.as_array()) {
                    current_span.record("images.success", photo_ids.len());
                }
                if let Some(failed) = result.output.get("failed_urls").and_then(|v| v.as_array()) {
                    current_span.record("images.failed", failed.len());
                }
            }
            "save_recipe" => {
                if let Some(recipe_id) = result.output.get("recipe_id").and_then(|v| v.as_str()) {
                    current_span.record("recipe.id", recipe_id);
                }
                // Get title from extract_recipe output
                if let Some(extract_output) = store.get_output("extract_recipe") {
                    if let Some(title) = extract_output
                        .get("raw_recipe")
                        .and_then(|r| r.get("title"))
                        .and_then(|t| t.as_str())
                    {
                        current_span.record("recipe.title", title);
                    }
                }
            }
            "enrich_normalize_ingredients" | "enrich_auto_tag" | "enrich_generate_photo" => {
                // Get recipe_id from save_recipe output
                if let Some(save_output) = store.get_output("save_recipe") {
                    if let Some(recipe_id) = save_output.get("recipe_id").and_then(|v| v.as_str()) {
                        current_span.record("recipe.id", recipe_id);
                    }
                }
            }
            _ => {}
        }

        result
    }
    .instrument(span)
    .await;

    result
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
