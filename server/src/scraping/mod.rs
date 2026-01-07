use crate::db::DbPool;
use crate::models::{NewRecipe, NewScrapeJob, NewStepOutput, ScrapeJob, StepOutput};
use crate::schema::{recipes, scrape_jobs, step_outputs};
use chrono::Utc;
use diesel::prelude::*;
use ramekin_core::{ExtractRecipeOutput, FetchHtmlOutput, RawRecipe, BUILD_ID};
use std::env;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

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

/// Step names for pipeline
const STEP_FETCH_HTML: &str = "fetch_html";
const STEP_EXTRACT_RECIPE: &str = "extract_recipe";
const STEP_SAVE_RECIPE: &str = "save_recipe";

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

/// Create a recipe from RawRecipe.
fn create_recipe_from_raw(
    pool: &DbPool,
    user_id: Uuid,
    raw: &RawRecipe,
) -> Result<Uuid, ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    // Convert raw ingredients (newline-separated blob) to our Ingredient JSON format
    let ingredients: Vec<crate::models::Ingredient> = raw
        .ingredients
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| crate::models::Ingredient {
            item: line.trim().to_string(),
            amount: None,
            unit: None,
            note: None,
        })
        .collect();

    let ingredients_json =
        serde_json::to_value(&ingredients).map_err(|e| ScrapeError::Database(e.to_string()))?;

    let new_recipe = NewRecipe {
        user_id,
        title: &raw.title,
        description: raw.description.as_deref(),
        ingredients: ingredients_json,
        instructions: &raw.instructions,
        source_url: Some(&raw.source_url),
        source_name: raw.source_name.as_deref(),
        photo_ids: &[],
        tags: &[],
    };

    let recipe_id: Uuid = diesel::insert_into(recipes::table)
        .values(&new_recipe)
        .returning(recipes::id)
        .get_result(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(recipe_id)
}

/// Run the scrape job state machine.
/// This processes the job through its states: pending -> scraping -> parsing -> completed
pub async fn run_scrape_job(pool: Arc<DbPool>, job_id: Uuid) {
    if let Err(e) = run_scrape_job_inner(&pool, job_id).await {
        tracing::warn!("Scrape job {} failed: {}", job_id, e);
    }
}

async fn run_scrape_job_inner(pool: &DbPool, job_id: Uuid) -> Result<(), ScrapeError> {
    let job = get_job(pool, job_id)?;

    match job.status.as_str() {
        STATUS_PENDING => {
            tracing::info!("Job {} transitioning from pending to scraping", job_id);
            update_status_and_step(pool, job_id, STATUS_SCRAPING, Some(STEP_FETCH_HTML))?;
            Box::pin(run_scrape_job_inner(pool, job_id)).await
        }

        STATUS_SCRAPING => {
            let current_step = job.current_step.as_deref().unwrap_or(STEP_FETCH_HTML);

            if current_step == STEP_FETCH_HTML {
                tracing::info!("Job {} fetching URL: {}", job_id, job.url);

                // Check host allowlist
                is_host_allowed(&job.url)?;

                match ramekin_core::fetch_html(&job.url).await {
                    Ok(html) => {
                        // Store fetch output
                        let fetch_output = FetchHtmlOutput { html };
                        let output_json = serde_json::to_value(&fetch_output)
                            .map_err(|e| ScrapeError::Database(e.to_string()))?;
                        save_step_output(pool, job_id, STEP_FETCH_HTML, output_json)?;

                        tracing::info!("Job {} fetch successful, transitioning to parsing", job_id);
                        update_status_and_step(
                            pool,
                            job_id,
                            STATUS_PARSING,
                            Some(STEP_EXTRACT_RECIPE),
                        )?;
                        Box::pin(run_scrape_job_inner(pool, job_id)).await
                    }
                    Err(e) => {
                        tracing::warn!("Job {} fetch failed: {}", job_id, e);
                        mark_failed(pool, job_id, STATUS_SCRAPING, &e.to_string())?;
                        Ok(())
                    }
                }
            } else {
                Err(ScrapeError::InvalidState(format!(
                    "Unexpected step in scraping status: {}",
                    current_step
                )))
            }
        }

        STATUS_PARSING => {
            let current_step = job.current_step.as_deref().unwrap_or(STEP_EXTRACT_RECIPE);
            tracing::info!("Job {} parsing, current_step: {}", job_id, current_step);

            if current_step == STEP_EXTRACT_RECIPE {
                // Get HTML from most recent step_output
                let fetch_output = get_latest_step_output(pool, job_id, STEP_FETCH_HTML)?
                    .ok_or_else(|| {
                        ScrapeError::InvalidState("No fetch output found".to_string())
                    })?;

                let html = fetch_output
                    .output
                    .get("html")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ScrapeError::InvalidState("No HTML in fetch output".to_string())
                    })?;

                match ramekin_core::extract_recipe(html, &job.url) {
                    Ok(raw_recipe) => {
                        // Store extract output
                        let extract_output = ExtractRecipeOutput { raw_recipe };
                        let output_json = serde_json::to_value(&extract_output)
                            .map_err(|e| ScrapeError::Database(e.to_string()))?;
                        save_step_output(pool, job_id, STEP_EXTRACT_RECIPE, output_json)?;

                        // Update current_step and continue
                        update_status_and_step(
                            pool,
                            job_id,
                            STATUS_PARSING,
                            Some(STEP_SAVE_RECIPE),
                        )?;

                        tracing::info!("Job {} extracted recipe, saving", job_id);
                        Box::pin(run_scrape_job_inner(pool, job_id)).await
                    }
                    Err(e) => {
                        tracing::warn!("Job {} parse failed: {}", job_id, e);
                        mark_failed(pool, job_id, STATUS_PARSING, &e.to_string())?;
                        Ok(())
                    }
                }
            } else if current_step == STEP_SAVE_RECIPE {
                // Get raw_recipe from most recent step_output
                let extract_output = get_latest_step_output(pool, job_id, STEP_EXTRACT_RECIPE)?
                    .ok_or_else(|| {
                        ScrapeError::InvalidState("No extract output found".to_string())
                    })?;

                let raw_recipe: RawRecipe = extract_output
                    .output
                    .get("raw_recipe")
                    .ok_or_else(|| {
                        ScrapeError::InvalidState("No raw_recipe in extract output".to_string())
                    })
                    .and_then(|v| {
                        serde_json::from_value(v.clone())
                            .map_err(|e| ScrapeError::Database(e.to_string()))
                    })?;

                tracing::info!("Job {} saving recipe: {}", job_id, raw_recipe.title);
                match create_recipe_from_raw(pool, job.user_id, &raw_recipe) {
                    Ok(recipe_id) => {
                        tracing::info!(
                            "Job {} created recipe {}, marking completed",
                            job_id,
                            recipe_id
                        );
                        mark_completed(pool, job_id, recipe_id)?;
                        Ok(())
                    }
                    Err(e) => {
                        tracing::error!("Job {} recipe creation failed: {}", job_id, e);
                        mark_failed(pool, job_id, STATUS_PARSING, &e.to_string())?;
                        Ok(())
                    }
                }
            } else {
                Err(ScrapeError::InvalidState(format!(
                    "Unexpected step in parsing status: {}",
                    current_step
                )))
            }
        }

        STATUS_COMPLETED | STATUS_FAILED => {
            // Terminal states - nothing to do
            Ok(())
        }

        other => Err(ScrapeError::InvalidState(other.to_string())),
    }
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
    let has_fetch_output = get_latest_step_output(pool, job_id, STEP_FETCH_HTML)?.is_some();
    let has_extract_output = get_latest_step_output(pool, job_id, STEP_EXTRACT_RECIPE)?.is_some();

    let (resume_status, resume_step) = match job.failed_at_step.as_deref() {
        Some(STATUS_SCRAPING) => {
            // Failed during fetch - restart from fetch
            (STATUS_SCRAPING, STEP_FETCH_HTML)
        }
        Some(STATUS_PARSING) => {
            if has_extract_output {
                // Have extract output, try save again
                (STATUS_PARSING, STEP_SAVE_RECIPE)
            } else if has_fetch_output {
                // Have fetch output, try extract again
                (STATUS_PARSING, STEP_EXTRACT_RECIPE)
            } else {
                // No outputs, start from beginning
                (STATUS_SCRAPING, STEP_FETCH_HTML)
            }
        }
        _ => {
            // Unknown failure point, start from beginning
            (STATUS_PENDING, STEP_FETCH_HTML)
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
