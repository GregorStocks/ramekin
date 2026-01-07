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

/// Update job status.
fn update_status(pool: &DbPool, job_id: Uuid, status: &str) -> Result<(), ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    diesel::update(scrape_jobs::table.find(job_id))
        .set((
            scrape_jobs::status.eq(status),
            scrape_jobs::updated_at.eq(Utc::now()),
        ))
        .execute(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(())
}

/// Save a step output to the database.
fn save_step_output(
    pool: &DbPool,
    job_id: Uuid,
    step_name: &str,
    output: serde_json::Value,
    next_step: Option<&str>,
) -> Result<(), ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    let new_output = NewStepOutput {
        scrape_job_id: job_id,
        step_name: step_name.to_string(),
        build_id: BUILD_ID.to_string(),
        output,
        next_step: next_step.map(|s| s.to_string()),
    };

    // Use ON CONFLICT to upsert (replace if same job/step/build exists)
    diesel::insert_into(step_outputs::table)
        .values(&new_output)
        .on_conflict((
            step_outputs::scrape_job_id,
            step_outputs::step_name,
            step_outputs::build_id,
        ))
        .do_update()
        .set((
            step_outputs::output.eq(&new_output.output),
            step_outputs::next_step.eq(&new_output.next_step),
        ))
        .execute(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(())
}

/// Get step output for a job by step name (with current build_id).
fn get_step_output(
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
        .filter(step_outputs::build_id.eq(BUILD_ID))
        .first::<StepOutput>(&mut conn)
        .optional()
        .map_err(|e| ScrapeError::Database(e.to_string()))
}

/// Check if any step outputs exist for this job (regardless of build_id).
fn has_any_step_outputs(pool: &DbPool, job_id: Uuid) -> Result<bool, ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    let count: i64 = step_outputs::table
        .filter(step_outputs::scrape_job_id.eq(job_id))
        .count()
        .get_result(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(count > 0)
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
            scrape_jobs::updated_at.eq(Utc::now()),
        ))
        .execute(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(())
}

/// Increment retry count and check if max exceeded.
fn increment_retry(pool: &DbPool, job_id: Uuid) -> Result<i32, ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    let new_count: i32 = diesel::update(scrape_jobs::table.find(job_id))
        .set((
            scrape_jobs::retry_count.eq(scrape_jobs::retry_count + 1),
            scrape_jobs::updated_at.eq(Utc::now()),
        ))
        .returning(scrape_jobs::retry_count)
        .get_result(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(new_count)
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

/// Determine the next step to run based on completed step outputs.
fn get_next_step(pool: &DbPool, job_id: Uuid) -> Result<&'static str, ScrapeError> {
    // Check steps in order - if a step with current build exists, use its next_step
    if let Some(extract_output) = get_step_output(pool, job_id, STEP_EXTRACT_RECIPE)? {
        if extract_output.next_step.as_deref() == Some(STEP_SAVE_RECIPE) {
            return Ok(STEP_SAVE_RECIPE);
        }
    }

    if let Some(fetch_output) = get_step_output(pool, job_id, STEP_FETCH_HTML)? {
        if fetch_output.next_step.as_deref() == Some(STEP_EXTRACT_RECIPE) {
            return Ok(STEP_EXTRACT_RECIPE);
        }
    }

    // Default: start from fetch
    Ok(STEP_FETCH_HTML)
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
            update_status(pool, job_id, STATUS_SCRAPING)?;
            Box::pin(run_scrape_job_inner(pool, job_id)).await
        }

        STATUS_SCRAPING => {
            // Check for stale step outputs from previous build
            let has_current_build_fetch = get_step_output(pool, job_id, STEP_FETCH_HTML)?.is_some();
            let has_any_outputs = has_any_step_outputs(pool, job_id)?;

            if !has_current_build_fetch && has_any_outputs {
                // Stale data detected (outputs exist but not for current build), increment retry
                tracing::info!(
                    "Job {} has stale step_outputs (build mismatch), restarting",
                    job_id
                );
                let retry_count = increment_retry(pool, job_id)?;
                if retry_count >= MAX_RETRIES {
                    mark_failed(pool, job_id, STATUS_SCRAPING, "Max retries exceeded")?;
                    return Err(ScrapeError::MaxRetriesExceeded);
                }
                // Continue with fresh execution
            }

            let next_step = get_next_step(pool, job_id)?;

            if next_step == STEP_FETCH_HTML {
                tracing::info!("Job {} fetching URL: {}", job_id, job.url);

                // Check host allowlist
                is_host_allowed(&job.url)?;

                match ramekin_core::fetch_html(&job.url).await {
                    Ok(html) => {
                        // Store fetch output
                        let fetch_output = FetchHtmlOutput { html };
                        let output_json = serde_json::to_value(&fetch_output)
                            .map_err(|e| ScrapeError::Database(e.to_string()))?;
                        save_step_output(
                            pool,
                            job_id,
                            STEP_FETCH_HTML,
                            output_json,
                            Some(STEP_EXTRACT_RECIPE),
                        )?;

                        tracing::info!("Job {} fetch successful, transitioning to parsing", job_id);
                        update_status(pool, job_id, STATUS_PARSING)?;
                        Box::pin(run_scrape_job_inner(pool, job_id)).await
                    }
                    Err(e) => {
                        tracing::warn!("Job {} fetch failed: {}", job_id, e);
                        mark_failed(pool, job_id, STATUS_SCRAPING, &e.to_string())?;
                        Ok(())
                    }
                }
            } else {
                // We have valid fetch data, move to parsing
                update_status(pool, job_id, STATUS_PARSING)?;
                Box::pin(run_scrape_job_inner(pool, job_id)).await
            }
        }

        STATUS_PARSING => {
            tracing::info!("Job {} parsing HTML", job_id);

            let next_step = get_next_step(pool, job_id)?;

            if next_step == STEP_EXTRACT_RECIPE {
                // Get HTML from step_outputs
                let fetch_output =
                    get_step_output(pool, job_id, STEP_FETCH_HTML)?.ok_or_else(|| {
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
                        save_step_output(
                            pool,
                            job_id,
                            STEP_EXTRACT_RECIPE,
                            output_json,
                            Some(STEP_SAVE_RECIPE),
                        )?;

                        tracing::info!("Job {} extracted recipe, saving", job_id);
                        // Continue to save step
                        Box::pin(run_scrape_job_inner(pool, job_id)).await
                    }
                    Err(e) => {
                        tracing::warn!("Job {} parse failed: {}", job_id, e);
                        mark_failed(pool, job_id, STATUS_PARSING, &e.to_string())?;
                        Ok(())
                    }
                }
            } else if next_step == STEP_SAVE_RECIPE {
                // Get raw_recipe from step_outputs
                let extract_output = get_step_output(pool, job_id, STEP_EXTRACT_RECIPE)?
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
                    "Unexpected next step: {}",
                    next_step
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

    // Check if we have valid step outputs to resume from
    let has_valid_fetch = get_step_output(pool, job_id, STEP_FETCH_HTML)?.is_some();
    let resume_status = match (job.failed_at_step.as_deref(), has_valid_fetch) {
        (Some(STATUS_SCRAPING), true) => {
            // We have valid fetch data, can skip to parsing
            STATUS_PARSING
        }
        (Some(STATUS_PARSING), _) => STATUS_PARSING,
        _ => STATUS_PENDING,
    };

    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    diesel::update(scrape_jobs::table.find(job_id))
        .set((
            scrape_jobs::status.eq(resume_status),
            scrape_jobs::failed_at_step.eq::<Option<String>>(None),
            scrape_jobs::error_message.eq::<Option<String>>(None),
            scrape_jobs::retry_count.eq(job.retry_count + 1),
            scrape_jobs::updated_at.eq(Utc::now()),
        ))
        .execute(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(resume_status.to_string())
}
