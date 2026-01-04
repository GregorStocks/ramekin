pub mod fetch;
pub mod parse;

use crate::db::DbPool;
use crate::models::{NewRecipe, NewScrapeJob, ScrapeJob};
use crate::schema::{recipes, scrape_jobs};
use chrono::Utc;
use diesel::prelude::*;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

pub use fetch::FetchError;
pub use parse::{ParseError, ParsedRecipe};

#[derive(Error, Debug)]
pub enum ScrapeError {
    #[error("Fetch error: {0}")]
    Fetch(#[from] FetchError),

    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Job not found")]
    JobNotFound,

    #[error("Invalid job state: {0}")]
    InvalidState(String),
}

/// Job statuses
pub const STATUS_PENDING: &str = "pending";
pub const STATUS_SCRAPING: &str = "scraping";
pub const STATUS_PARSING: &str = "parsing";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_FAILED: &str = "failed";

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

/// Create a recipe from parsed data.
fn create_recipe_from_parsed(
    pool: &DbPool,
    user_id: Uuid,
    parsed: &ParsedRecipe,
    source_url: &str,
) -> Result<Uuid, ScrapeError> {
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    let ingredients_json = serde_json::to_value(&parsed.ingredients)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    let new_recipe = NewRecipe {
        user_id,
        title: &parsed.title,
        description: parsed.description.as_deref(),
        ingredients: ingredients_json,
        instructions: &parsed.instructions,
        source_url: Some(source_url),
        source_name: parsed.source_name.as_deref(),
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
        tracing::error!("Scrape job {} failed: {}", job_id, e);
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
            tracing::info!("Job {} scraping URL: {}", job_id, job.url);
            match fetch::fetch_url(pool, &job.url).await {
                Ok(_html) => {
                    tracing::info!("Job {} fetch successful, transitioning to parsing", job_id);
                    update_status(pool, job_id, STATUS_PARSING)?;
                    Box::pin(run_scrape_job_inner(pool, job_id)).await
                }
                Err(e) => {
                    tracing::error!("Job {} fetch failed: {}", job_id, e);
                    mark_failed(pool, job_id, STATUS_SCRAPING, &e.to_string())?;
                    Ok(())
                }
            }
        }

        STATUS_PARSING => {
            tracing::info!("Job {} parsing HTML", job_id);
            // Get HTML from cache (it was just saved in scraping step)
            let cached = fetch::get_cached(pool, &job.url)?
                .ok_or_else(|| ScrapeError::Database("Cache miss after scraping".to_string()))?;

            let html = String::from_utf8(cached.content)
                .map_err(|e| ScrapeError::Database(format!("Invalid UTF-8: {}", e)))?;

            match parse::parse_recipe_from_html(&html, &job.url) {
                Ok(parsed) => {
                    tracing::info!("Job {} parsed recipe: {}", job_id, parsed.title);
                    match create_recipe_from_parsed(pool, job.user_id, &parsed, &job.url) {
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
                }
                Err(e) => {
                    tracing::error!("Job {} parse failed: {}", job_id, e);
                    mark_failed(pool, job_id, STATUS_PARSING, &e.to_string())?;
                    Ok(())
                }
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

    let resume_status = match job.failed_at_step.as_deref() {
        Some(STATUS_SCRAPING) => {
            // If we have cached HTML, skip to parsing
            if fetch::get_cached(pool, &job.url)?.is_some() {
                STATUS_PARSING
            } else {
                STATUS_SCRAPING
            }
        }
        Some(STATUS_PARSING) => STATUS_PARSING,
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
