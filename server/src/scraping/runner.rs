use std::sync::Arc;

use diesel::prelude::*;
use tracing::Instrument;
use uuid::Uuid;

use ramekin_core::ai::{AiClient, CachingAiClient};
use ramekin_core::pipeline::steps::{
    EnrichAutoTagStep, FetchImagesStepMeta, ParseIngredientsStep, SaveRecipeStepMeta,
};
use ramekin_core::pipeline::{
    scrape_auto_applied_ai_enrichments, scrape_pipeline_step_names, PipelineStep,
    ScrapeAutoAppliedAiEnrichment, StepContext, StepOutputStore, StepRegistry,
};

use crate::db::DbPool;
use crate::schema::{scrape_jobs, step_outputs, user_tags};

use super::jobs::{get_job, mark_completed, mark_failed, update_status_and_step};
use super::output_store::DbOutputStore;
use super::steps::{
    ApplyAutoTagsStep, ApplyGeneratedDescriptionStep, ApplyNormalizedTitleStep, FetchHtmlStep,
    FetchImagesStep, SaveRecipeStep,
};
use super::{
    run_scrape_db, ScrapeError, STATUS_COMPLETED, STATUS_FAILED, STATUS_PARSING, STATUS_SCRAPING,
};

/// Maximum retries before hard fail
const MAX_RETRIES: i32 = 5;

/// Fetch user's existing tags from the database.
async fn fetch_user_tags(pool: &DbPool, user_id: Uuid) -> Result<Vec<String>, ScrapeError> {
    run_scrape_db(pool, move |conn| {
        user_tags::table
            .filter(user_tags::user_id.eq(user_id))
            .filter(user_tags::deleted_at.is_null())
            .select(user_tags::name)
            .order(user_tags::name.asc())
            .load::<String>(conn)
            .map_err(|e| ScrapeError::Database(e.to_string()))
    })
    .await
}

/// Build a step registry for server-side pipeline execution.
///
/// This creates all step implementations with the necessary resources (DB pool, user ID).
/// If `existing_recipe_id` is provided, SaveRecipeStep will update that recipe instead of
/// creating a new one (for rescrape functionality).
pub async fn build_registry(
    pool: Arc<DbPool>,
    user_id: Uuid,
    existing_recipe_id: Option<Uuid>,
    expected_version_id: Option<Uuid>,
    photo_only: bool,
) -> Result<StepRegistry, ScrapeError> {
    let mut registry = StepRegistry::new();
    registry.register(Box::new(FetchHtmlStep));
    registry.register(Box::new(ramekin_core::pipeline::steps::ExtractRecipeStep));
    registry.register(Box::new(FetchImagesStep::new(pool.clone(), user_id)));
    registry.register(Box::new(ParseIngredientsStep));

    // Pick the right SaveRecipeStep based on job mode.
    let save_step = match (existing_recipe_id, expected_version_id, photo_only) {
        (Some(recipe_id), Some(expected_version_id), true) => SaveRecipeStep::for_photo_rescrape(
            pool.clone(),
            user_id,
            recipe_id,
            expected_version_id,
        ),
        (Some(recipe_id), Some(expected_version_id), false) => {
            SaveRecipeStep::for_rescrape(pool.clone(), user_id, recipe_id, expected_version_id)
        }
        (Some(_), None, _) => {
            return Err(ScrapeError::InvalidState(
                "rescrape job has no expected recipe version".to_string(),
            ));
        }
        (None, None, _) => SaveRecipeStep::new(pool.clone(), user_id),
        (None, Some(_), _) => {
            return Err(ScrapeError::InvalidState(
                "new recipe job unexpectedly has an expected recipe version".to_string(),
            ));
        }
    };
    registry.register(Box::new(save_step));

    let auto_enrichments = scrape_auto_applied_ai_enrichments();
    let ai_client = if auto_enrichments.is_empty() {
        None
    } else {
        Some(Arc::new(CachingAiClient::from_env()?) as Arc<dyn AiClient>)
    };

    for enrichment in auto_enrichments {
        match enrichment {
            ScrapeAutoAppliedAiEnrichment::NormalizeTitle => {
                registry.register(Box::new(
                    ramekin_core::pipeline::steps::EnrichNormalizeTitleStep::new(
                        ai_client
                            .as_ref()
                            .expect("AI client exists for auto enrichment")
                            .clone(),
                    ),
                ));
                registry.register(Box::new(ApplyNormalizedTitleStep::new(pool.clone())));
            }
            ScrapeAutoAppliedAiEnrichment::GenerateDescription => {
                registry.register(Box::new(
                    ramekin_core::pipeline::steps::EnrichGenerateDescriptionStep::new(
                        ai_client
                            .as_ref()
                            .expect("AI client exists for auto enrichment")
                            .clone(),
                    ),
                ));
                registry.register(Box::new(ApplyGeneratedDescriptionStep::new(pool.clone())));
            }
            ScrapeAutoAppliedAiEnrichment::AutoTag => {
                let user_tags = fetch_user_tags(&pool, user_id).await?;

                registry.register(Box::new(EnrichAutoTagStep::new(
                    ai_client
                        .as_ref()
                        .expect("AI client exists for auto enrichment")
                        .clone(),
                    user_tags,
                )));
                registry.register(Box::new(ApplyAutoTagsStep::new(pool.clone())));
            }
        }
    }
    Ok(registry)
}

/// Spawn an import job (same as scrape job, but no URL for tracing).
pub fn spawn_import_job(pool: Arc<DbPool>, job_id: Uuid) {
    let span = tracing::info_span!(
        "import_job",
        otel.name = "import_job",
        job.id = %job_id,
        job.operation = "import",
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

#[derive(Debug, PartialEq, Eq)]
enum ScrapeFinalOutcome {
    Completed(Uuid),
    Failed { step_name: String, error: String },
}

fn determine_final_outcome(
    recipe_id: Option<Uuid>,
    terminal_error: Option<(String, String)>,
    last_step_name: &str,
    last_error: Option<String>,
) -> ScrapeFinalOutcome {
    if let Some((step_name, error)) = terminal_error {
        return ScrapeFinalOutcome::Failed { step_name, error };
    }

    if let Some(id) = recipe_id {
        return ScrapeFinalOutcome::Completed(id);
    }

    if let Some(error) = last_error {
        return ScrapeFinalOutcome::Failed {
            step_name: last_step_name.to_string(),
            error,
        };
    }

    ScrapeFinalOutcome::Failed {
        step_name: last_step_name.to_string(),
        error: "Pipeline ended without creating recipe".to_string(),
    }
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
    let job = get_job(&pool, job_id).await?;

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

    // Build the step registry and output store.
    // If job.recipe_id is already set, this is a rescrape - pass it to build_registry.
    let registry = match build_registry(
        pool.clone(),
        job.user_id,
        job.recipe_id,
        job.expected_version_id,
        job.photo_only,
    )
    .await
    {
        Ok(registry) => registry,
        Err(e) => {
            mark_failed(&pool, job_id, first_step, &e.to_string()).await?;
            return Err(e);
        }
    };
    let mut store = DbOutputStore::new(&pool, job_id);

    // URL for context (empty string for imports without a URL)
    let url = job.url.as_deref().unwrap_or("");

    // Run pipeline with status updates and OpenTelemetry instrumentation
    let mut current_step_name: Option<String> = Some(first_step.to_string());
    // Real step name of the most recently executed step. Used to populate
    // `scrape_jobs.failed_at_step` if the pipeline ends without a recipe.
    let mut last_step_name: String = first_step.to_string();
    let mut last_error: Option<String> = None;
    let mut terminal_error: Option<(String, String)> = None;

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
        last_step_name = step_name.clone();

        // Update job status and current_step before executing
        update_status_and_step(&pool, job_id, step_status, Some(&step_name)).await?;

        // Execute step with OpenTelemetry span
        let mut result = execute_step_with_tracing(step, url, &store, &step_name).await;

        let meta = step.metadata();

        // Save output (for both success and failure - useful for debugging).
        // If persistence itself fails we normally fail the step: silently
        // swallowing the error leaves a "successful" step with no output row,
        // which makes downstream retries think the step already ran and
        // produces a deadlock where the pipeline can never make progress.
        //
        // Exception: for `continues_on_failure` steps (enrichment), a
        // persistence failure should NOT terminate the pipeline — otherwise a
        // transient save error after `save_recipe` has already succeeded
        // would cause the user to silently lose enrichment for that recipe.
        // We still mark the step as failed on the result so the persisted row
        // (and status API) reflect the failure, but we preserve `next_step`
        // so the chain continues.
        if let Err(e) = store
            .save_output(
                &step_name,
                &result.output,
                result.duration_ms as i64,
                result.success,
                result.error.as_deref(),
            )
            .await
        {
            let msg = format!("Failed to persist output for step {step_name}: {e}");
            tracing::error!("{msg}");
            result.success = false;
            result.error = Some(msg);
            if !meta.continues_on_failure {
                result.next_step = None;
            }
        }

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
            terminal_error = Some((
                step_name.clone(),
                result
                    .error
                    .clone()
                    .unwrap_or_else(|| "Step failed".to_string()),
            ));
            break;
        }

        current_step_name = result.next_step;
    }

    // Determine final outcome
    let recipe_id = store
        .get_output(SaveRecipeStepMeta::NAME)
        .await
        .and_then(|o| {
            o.get("recipe_id")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .and_then(|s| Uuid::parse_str(&s).ok());

    match determine_final_outcome(recipe_id, terminal_error, &last_step_name, last_error) {
        ScrapeFinalOutcome::Completed(id) => {
            tracing::info!("Job {} completed successfully, recipe_id={}", job_id, id);
            mark_completed(&pool, job_id, id).await?;
        }
        ScrapeFinalOutcome::Failed { step_name, error } => {
            tracing::warn!("Job {} failed at '{}': {}", job_id, step_name, error);
            mark_failed(&pool, job_id, &step_name, &error).await?;
        }
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

    async {
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
                if let Some(extract_output) = store.get_output("extract_recipe").await {
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
                if let Some(extract_output) = store.get_output("extract_recipe").await {
                    if let Some(title) = extract_output
                        .get("raw_recipe")
                        .and_then(|r| r.get("title"))
                        .and_then(|t| t.as_str())
                    {
                        current_span.record("recipe.title", title);
                    }
                }
            }
            "enrich_auto_tag" | "apply_auto_tags" => {
                // Get recipe_id from save_recipe output
                if let Some(save_output) = store.get_output("save_recipe").await {
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
    .await
}

/// Reset a failed job for retry.
/// Returns the status to resume from.
pub async fn retry_job(pool: &DbPool, job_id: Uuid) -> Result<String, ScrapeError> {
    let job = get_job(pool, job_id).await?;

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

    run_scrape_db(pool, move |conn| {
        // Resume at the step that failed. `failed_at_step` holds the real pipeline
        // step name (see `mark_failed`); if it's missing or not a canonical pipeline
        // step, fall back to running the whole pipeline from the top.
        let mut resume_step: String = job
            .failed_at_step
            .as_deref()
            .filter(|name| scrape_pipeline_step_names().contains(name))
            .unwrap_or(FetchHtmlStep::NAME)
            .to_string();

        // Photo-only jobs: if the failed step is save_recipe or later but
        // fetch_images produced an empty photo_ids set, resuming at the failed
        // step reuses the stale empty fetch_images output and save_recipe rejects
        // it again — creating an unrecoverable retry loop. Rewind to fetch_images
        // so it actually retries the image download. Narrowly scoped to
        // photo-only jobs; full scrapes have their own empty-photo handling.
        if job.photo_only {
            let pipeline_steps = scrape_pipeline_step_names();
            let resume_idx = pipeline_steps
                .iter()
                .position(|s| *s == resume_step.as_str());
            let save_idx = pipeline_steps
                .iter()
                .position(|s| *s == SaveRecipeStepMeta::NAME);
            if let (Some(ri), Some(si)) = (resume_idx, save_idx) {
                if ri >= si {
                    let fetch_images_output: Option<serde_json::Value> = step_outputs::table
                        .filter(step_outputs::scrape_job_id.eq(job_id))
                        .filter(step_outputs::step_name.eq(FetchImagesStepMeta::NAME))
                        .select(step_outputs::output)
                        .order(step_outputs::created_at.desc())
                        .first::<serde_json::Value>(conn)
                        .optional()
                        .map_err(|e| ScrapeError::Database(e.to_string()))?;

                    let photo_ids_empty = fetch_images_output
                        .as_ref()
                        .and_then(|o: &serde_json::Value| o.get("photo_ids"))
                        .and_then(|v: &serde_json::Value| v.as_array())
                        .map(|a: &Vec<serde_json::Value>| a.is_empty())
                        .unwrap_or(false);

                    if photo_ids_empty {
                        tracing::info!(
                            job_id = %job_id,
                            original_resume = %resume_step,
                            "rewinding photo-only retry to fetch_images (empty photo_ids)"
                        );
                        resume_step = FetchImagesStepMeta::NAME.to_string();
                    }
                }
            }
        }

        let resume_status = if resume_step.as_str() == FetchHtmlStep::NAME {
            STATUS_SCRAPING
        } else {
            STATUS_PARSING
        };

        let now = chrono::Utc::now();
        diesel::update(scrape_jobs::table.find(job_id))
            .set((
                scrape_jobs::status.eq(resume_status),
                scrape_jobs::current_step.eq(Some(resume_step.as_str())),
                scrape_jobs::current_step_started_at.eq(Some(now)),
                scrape_jobs::failed_at_step.eq::<Option<String>>(None),
                scrape_jobs::error_message.eq::<Option<String>>(None),
                scrape_jobs::retry_count.eq(job.retry_count + 1),
                scrape_jobs::updated_at.eq(now),
            ))
            .execute(conn)
            .map_err(|e| ScrapeError::Database(e.to_string()))?;

        Ok(resume_status.to_string())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_failure_wins_over_saved_recipe() {
        let recipe_id = Uuid::new_v4();

        let outcome = determine_final_outcome(
            Some(recipe_id),
            Some((
                "apply_normalized_title".to_string(),
                "database exploded".to_string(),
            )),
            "apply_normalized_title",
            Some("database exploded".to_string()),
        );

        assert_eq!(
            outcome,
            ScrapeFinalOutcome::Failed {
                step_name: "apply_normalized_title".to_string(),
                error: "database exploded".to_string(),
            }
        );
    }
}
