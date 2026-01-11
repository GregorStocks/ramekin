use crate::db::DbPool;
use crate::models::{NewPhoto, NewRecipe, NewScrapeJob, NewStepOutput, ScrapeJob, StepOutput};
use crate::photos::processing::{process_image, MAX_FILE_SIZE};
use crate::schema::{photos, recipes, scrape_jobs, step_outputs};
use chrono::Utc;
use diesel::prelude::*;
use ramekin_core::{
    ExtractRecipeOutput, FailedImageFetch, FetchHtmlOutput, FetchImagesOutput, RawRecipe, BUILD_ID,
};
use std::env;
use std::sync::Arc;
use thiserror::Error;
use tracing::Instrument;
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
const STEP_FETCH_IMAGES: &str = "fetch_images";
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
    save_step_output(pool, job.id, STEP_FETCH_HTML, output_json)?;

    // Update the job to start from parsing (skip fetch step)
    diesel::update(scrape_jobs::table.find(job.id))
        .set((
            scrape_jobs::status.eq(STATUS_PARSING),
            scrape_jobs::current_step.eq(Some(STEP_EXTRACT_RECIPE)),
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

/// Create a recipe from RawRecipe.
pub fn create_recipe_from_raw(
    pool: &DbPool,
    user_id: Uuid,
    raw: &RawRecipe,
    photo_ids: &[Uuid],
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

    // Convert photo IDs to Option<Uuid> for the database
    let photo_ids_nullable: Vec<Option<Uuid>> = photo_ids.iter().map(|id| Some(*id)).collect();

    let new_recipe = NewRecipe {
        user_id,
        title: &raw.title,
        description: raw.description.as_deref(),
        ingredients: ingredients_json,
        instructions: &raw.instructions,
        source_url: Some(&raw.source_url),
        source_name: raw.source_name.as_deref(),
        photo_ids: &photo_ids_nullable,
        tags: &[],
        // Paprika-compatible fields - not populated from web scraping
        servings: None,
        prep_time: None,
        cook_time: None,
        total_time: None,
        rating: None,
        difficulty: None,
        nutritional_info: None,
        notes: None,
    };

    let recipe_id: Uuid = diesel::insert_into(recipes::table)
        .values(&new_recipe)
        .returning(recipes::id)
        .get_result(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    Ok(recipe_id)
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
    let result = run_scrape_job_inner(&pool, job_id).await;
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
                // Check host allowlist
                is_host_allowed(&job.url)?;

                let fetch_span = tracing::info_span!(
                    "scrape_step",
                    otel.name = "fetch_html",
                    step.name = "fetch_html",
                    http.url = %job.url,
                    http.response_content_length = tracing::field::Empty,
                );

                let fetch_result = async { ramekin_core::fetch_html(&job.url).await }
                    .instrument(fetch_span.clone())
                    .await;

                match fetch_result {
                    Ok(html) => {
                        fetch_span.record("http.response_content_length", html.len());

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

                let extract_span = tracing::info_span!(
                    "scrape_step",
                    otel.name = "extract_recipe",
                    step.name = "extract_recipe",
                    recipe.title = tracing::field::Empty,
                );

                let extract_result =
                    extract_span.in_scope(|| ramekin_core::extract_recipe(html, &job.url));

                match extract_result {
                    Ok(raw_recipe) => {
                        extract_span.record("recipe.title", raw_recipe.title.as_str());

                        // Store extract output
                        let extract_output = ExtractRecipeOutput { raw_recipe };
                        let output_json = serde_json::to_value(&extract_output)
                            .map_err(|e| ScrapeError::Database(e.to_string()))?;
                        save_step_output(pool, job_id, STEP_EXTRACT_RECIPE, output_json)?;

                        // Update current_step and continue to fetch images
                        update_status_and_step(
                            pool,
                            job_id,
                            STATUS_PARSING,
                            Some(STEP_FETCH_IMAGES),
                        )?;

                        tracing::info!("Job {} extracted recipe, fetching images", job_id);
                        Box::pin(run_scrape_job_inner(pool, job_id)).await
                    }
                    Err(e) => {
                        tracing::warn!("Job {} parse failed: {}", job_id, e);
                        mark_failed(pool, job_id, STATUS_PARSING, &e.to_string())?;
                        Ok(())
                    }
                }
            } else if current_step == STEP_FETCH_IMAGES {
                // Get raw_recipe from extract output to get image URLs
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

                let fetch_images_span = tracing::info_span!(
                    "scrape_step",
                    otel.name = "fetch_images",
                    step.name = "fetch_images",
                    images.requested = raw_recipe.image_urls.len(),
                    images.success = tracing::field::Empty,
                    images.failed = tracing::field::Empty,
                );

                // Fetch the first image (if any)
                let fetch_images_result = fetch_images_span
                    .in_scope(|| fetch_and_store_images(pool, job.user_id, &raw_recipe.image_urls))
                    .await;

                let output = match fetch_images_result {
                    Ok(output) => output,
                    Err(e) => {
                        // Even if something catastrophic happens, continue with empty photos
                        tracing::warn!("Image fetch failed entirely: {}", e);
                        FetchImagesOutput {
                            photo_ids: vec![],
                            failed_urls: vec![FailedImageFetch {
                                url: "all".to_string(),
                                error: e.to_string(),
                            }],
                        }
                    }
                };

                fetch_images_span.record("images.success", output.photo_ids.len());
                fetch_images_span.record("images.failed", output.failed_urls.len());

                let output_json = serde_json::to_value(&output)
                    .map_err(|e| ScrapeError::Database(e.to_string()))?;
                save_step_output(pool, job_id, STEP_FETCH_IMAGES, output_json)?;

                // Continue to save_recipe
                update_status_and_step(pool, job_id, STATUS_PARSING, Some(STEP_SAVE_RECIPE))?;
                tracing::info!(
                    "Job {} fetched {} images, saving recipe",
                    job_id,
                    output.photo_ids.len()
                );
                Box::pin(run_scrape_job_inner(pool, job_id)).await
            } else if current_step == STEP_SAVE_RECIPE {
                // Get raw_recipe from extract output
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

                // Get photo IDs from fetch_images output (may not exist for old jobs)
                let photo_ids: Vec<Uuid> = get_latest_step_output(pool, job_id, STEP_FETCH_IMAGES)?
                    .and_then(|output| {
                        output
                            .output
                            .get("photo_ids")
                            .and_then(|v| serde_json::from_value(v.clone()).ok())
                    })
                    .unwrap_or_default();

                let save_span = tracing::info_span!(
                    "scrape_step",
                    otel.name = "save_recipe",
                    step.name = "save_recipe",
                    recipe.title = %raw_recipe.title,
                    recipe.id = tracing::field::Empty,
                    recipe.photo_count = photo_ids.len(),
                );

                let save_result = save_span.in_scope(|| {
                    create_recipe_from_raw(pool, job.user_id, &raw_recipe, &photo_ids)
                });

                match save_result {
                    Ok(recipe_id) => {
                        save_span.record("recipe.id", tracing::field::display(recipe_id));
                        tracing::info!(
                            "Job {} created recipe {} with {} photos, marking completed",
                            job_id,
                            recipe_id,
                            photo_ids.len()
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

/// Fetch images from URLs, process them, and store as Photo records.
/// Returns photo IDs for successful downloads and errors for failed ones.
/// Only fetches the first image URL (if any).
async fn fetch_and_store_images(
    pool: &DbPool,
    user_id: Uuid,
    image_urls: &[String],
) -> Result<FetchImagesOutput, ScrapeError> {
    let mut photo_ids = Vec::new();
    let mut failed_urls = Vec::new();

    // Only fetch the first image
    if let Some(url) = image_urls.first() {
        match fetch_and_store_single_image(pool, user_id, url).await {
            Ok(photo_id) => photo_ids.push(photo_id),
            Err(e) => {
                tracing::warn!("Failed to fetch image {}: {}", url, e);
                failed_urls.push(FailedImageFetch {
                    url: url.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    Ok(FetchImagesOutput {
        photo_ids,
        failed_urls,
    })
}

/// Fetch a single image, process it, and store as a Photo record.
async fn fetch_and_store_single_image(
    pool: &DbPool,
    user_id: Uuid,
    url: &str,
) -> Result<Uuid, ScrapeError> {
    // Check if host is allowed
    is_host_allowed(url)?;

    // Fetch the image bytes
    let data = ramekin_core::fetch_bytes(url).await?;

    // Validate size
    if data.len() > MAX_FILE_SIZE {
        return Err(ScrapeError::InvalidState(format!(
            "Image too large: {} bytes (max {})",
            data.len(),
            MAX_FILE_SIZE
        )));
    }

    // Process: validate format, generate thumbnail
    let (content_type, thumbnail) = process_image(&data).map_err(ScrapeError::InvalidState)?;

    // Store in database
    let mut conn = pool
        .get()
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    let new_photo = NewPhoto {
        user_id,
        content_type: &content_type,
        data: &data,
        thumbnail: &thumbnail,
    };

    let photo_id: Uuid = diesel::insert_into(photos::table)
        .values(&new_photo)
        .returning(photos::id)
        .get_result(&mut conn)
        .map_err(|e| ScrapeError::Database(e.to_string()))?;

    tracing::info!("Stored photo {} from {}", photo_id, url);
    Ok(photo_id)
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
    let has_fetch_images_output =
        get_latest_step_output(pool, job_id, STEP_FETCH_IMAGES)?.is_some();

    let (resume_status, resume_step) = match job.failed_at_step.as_deref() {
        Some(STATUS_SCRAPING) => {
            // Failed during fetch - restart from fetch
            (STATUS_SCRAPING, STEP_FETCH_HTML)
        }
        Some(STATUS_PARSING) => {
            if has_fetch_images_output {
                // Have fetch_images output, try save again
                (STATUS_PARSING, STEP_SAVE_RECIPE)
            } else if has_extract_output {
                // Have extract output, try fetch_images
                (STATUS_PARSING, STEP_FETCH_IMAGES)
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
