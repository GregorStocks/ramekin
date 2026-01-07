use crate::db::DbPool;
use crate::models::{NewRecipe, NewScrapeJob, ScrapeJob};
use crate::schema::{recipes, scrape_jobs};
use chrono::Utc;
use diesel::prelude::*;
use ramekin_core::{ExtractRecipeOutput, FetchHtmlOutput, RawRecipe, StepOutput, BUILD_ID};
use serde_json::json;
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

/// Update job step_data.
fn update_step_data(
    pool: &DbPool,
    job_id: Uuid,
    step_data: serde_json::Value,
) -> Result<(), ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    diesel::update(scrape_jobs::table.find(job_id))
        .set((
            scrape_jobs::step_data.eq(step_data),
            scrape_jobs::updated_at.eq(Utc::now()),
        ))
        .execute(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(())
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
            scrape_jobs::step_data.eq(json!({})),
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

/// Get the last completed step from step_data, checking build_id.
/// Returns None if no steps completed or build_id mismatch.
fn get_last_valid_step(step_data: &Option<serde_json::Value>) -> Option<&'static str> {
    let step_data = step_data.as_ref()?;
    // Check steps in reverse order of the pipeline
    for step_name in [STEP_EXTRACT_RECIPE, STEP_FETCH_HTML] {
        if let Some(step) = step_data.get(step_name) {
            if let Some(build_id) = step.get("build_id").and_then(|v| v.as_str()) {
                if build_id == BUILD_ID {
                    return Some(step_name);
                }
            }
        }
    }
    None
}

/// Get the next step to run based on step_data.
fn get_next_step(step_data: &Option<serde_json::Value>) -> &'static str {
    let Some(step_data) = step_data.as_ref() else {
        return STEP_FETCH_HTML;
    };

    // Check what the last completed step says is next
    if let Some(extract_step) = step_data.get(STEP_EXTRACT_RECIPE) {
        if let Some(next) = extract_step.get("next_step").and_then(|v| v.as_str()) {
            if next == STEP_SAVE_RECIPE {
                return STEP_SAVE_RECIPE;
            }
        }
    }

    if let Some(fetch_step) = step_data.get(STEP_FETCH_HTML) {
        if let Some(next) = fetch_step.get("next_step").and_then(|v| v.as_str()) {
            if next == STEP_EXTRACT_RECIPE {
                return STEP_EXTRACT_RECIPE;
            }
        }
    }

    // Default: start from fetch
    STEP_FETCH_HTML
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
            // Check for stale step_data from previous build
            let last_valid = get_last_valid_step(&job.step_data);
            let has_step_data = job
                .step_data
                .as_ref()
                .map(|v| v != &json!({}))
                .unwrap_or(false);
            if last_valid.is_none() && has_step_data {
                // Stale data detected, restart
                tracing::info!(
                    "Job {} has stale step_data (build mismatch), restarting",
                    job_id
                );
                let retry_count = increment_retry(pool, job_id)?;
                if retry_count >= MAX_RETRIES {
                    mark_failed(pool, job_id, STATUS_SCRAPING, "Max retries exceeded")?;
                    return Err(ScrapeError::MaxRetriesExceeded);
                }
                // Continue with fresh step_data
            }

            let next_step = get_next_step(&job.step_data);

            if next_step == STEP_FETCH_HTML {
                tracing::info!("Job {} fetching URL: {}", job_id, job.url);

                // Check host allowlist
                is_host_allowed(&job.url)?;

                match ramekin_core::fetch_html(&job.url).await {
                    Ok(html) => {
                        // Store fetch output in step_data
                        let fetch_output: StepOutput<FetchHtmlOutput> = StepOutput {
                            build_id: BUILD_ID.to_string(),
                            output: FetchHtmlOutput { html },
                            next_step: Some(STEP_EXTRACT_RECIPE.to_string()),
                        };

                        let mut step_data = job.step_data.clone().unwrap_or_else(|| json!({}));
                        step_data[STEP_FETCH_HTML] = serde_json::to_value(&fetch_output)
                            .map_err(|e| ScrapeError::Database(e.to_string()))?;
                        update_step_data(pool, job_id, step_data)?;

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

            let next_step = get_next_step(&job.step_data);
            let step_data_ref = job.step_data.as_ref();

            if next_step == STEP_EXTRACT_RECIPE {
                // Get HTML from step_data
                let html = step_data_ref
                    .and_then(|sd| sd.get(STEP_FETCH_HTML))
                    .and_then(|v| v.get("output"))
                    .and_then(|v| v.get("html"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ScrapeError::InvalidState("No HTML in step_data".to_string()))?;

                match ramekin_core::extract_recipe(html, &job.url) {
                    Ok(raw_recipe) => {
                        // Store extract output in step_data
                        let extract_output: StepOutput<ExtractRecipeOutput> = StepOutput {
                            build_id: BUILD_ID.to_string(),
                            output: ExtractRecipeOutput { raw_recipe },
                            next_step: Some(STEP_SAVE_RECIPE.to_string()),
                        };

                        let mut step_data = job.step_data.clone().unwrap_or_else(|| json!({}));
                        step_data[STEP_EXTRACT_RECIPE] = serde_json::to_value(&extract_output)
                            .map_err(|e| ScrapeError::Database(e.to_string()))?;
                        update_step_data(pool, job_id, step_data)?;

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
                // Get raw_recipe from step_data
                let raw_recipe: RawRecipe = step_data_ref
                    .and_then(|sd| sd.get(STEP_EXTRACT_RECIPE))
                    .and_then(|v| v.get("output"))
                    .and_then(|v| v.get("raw_recipe"))
                    .ok_or_else(|| {
                        ScrapeError::InvalidState("No raw_recipe in step_data".to_string())
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

    // Check if we have valid step_data to resume from
    let last_valid = get_last_valid_step(&job.step_data);
    let resume_status = match (job.failed_at_step.as_deref(), last_valid) {
        (Some(STATUS_SCRAPING), Some(STEP_FETCH_HTML)) => {
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
