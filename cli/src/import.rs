use anyhow::{bail, Context, Result};
use base64::Engine;
use flate2::read::GzDecoder;
use ramekin_client::apis::configuration::Configuration;
use ramekin_client::apis::{auth_api, scrape_api};
use ramekin_client::models::LoginRequest;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::Duration;
use zip::ZipArchive;

/// Paprika recipe format
#[derive(Debug, Deserialize)]
struct PaprikaRecipe {
    name: String,
    ingredients: Option<String>,
    directions: Option<String>,
    description: Option<String>,
    notes: Option<String>,
    source: Option<String>,
    source_url: Option<String>,
    categories: Option<Vec<String>>,
    /// Full resolution photos array (preferred)
    #[serde(default)]
    photos: Vec<PaprikaPhoto>,
    /// Thumbnail/fallback photo (used when photos array is empty)
    photo_data: Option<String>,
    servings: Option<String>,
    prep_time: Option<String>,
    cook_time: Option<String>,
    total_time: Option<String>,
    rating: Option<i32>,
    difficulty: Option<String>,
    nutritional_info: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PaprikaPhoto {
    data: Option<String>,
}

/// Raw recipe data for import (matches server's ImportRawRecipe)
#[derive(Debug, Clone, Serialize)]
struct ImportRawRecipe {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    ingredients: String,
    instructions: String,
    #[serde(default)]
    image_urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    servings: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prep_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cook_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rating: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    difficulty: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nutritional_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    categories: Option<Vec<String>>,
}

/// Import request body
#[derive(Debug, Serialize)]
struct ImportRecipeRequest {
    raw_recipe: ImportRawRecipe,
    photo_ids: Vec<uuid::Uuid>,
    extraction_method: String,
}

/// Import response
#[derive(Debug, Deserialize)]
struct ImportRecipeResponse {
    job_id: uuid::Uuid,
    status: String,
}

/// Upload a photo via multipart form and return its UUID
pub async fn upload_photo(config: &Configuration, image_data: &[u8]) -> Result<uuid::Uuid> {
    upload_photo_with_client(config, image_data, &reqwest::Client::new()).await
}

/// Upload a photo via multipart form using a provided client and return its UUID
pub async fn upload_photo_with_client(
    config: &Configuration,
    image_data: &[u8],
    client: &reqwest::Client,
) -> Result<uuid::Uuid> {
    let part = reqwest::multipart::Part::bytes(image_data.to_vec())
        .file_name("image.jpg")
        .mime_str("image/jpeg")?;

    let form = reqwest::multipart::Form::new().part("file", part);

    let mut request = client
        .post(format!("{}/api/photos", config.base_path))
        .multipart(form);

    if let Some(ref token) = config.bearer_access_token {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .await
        .context("Failed to send photo upload request")?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "Photo upload failed with status {} ({}): {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or("Unknown"),
            body
        );
    }

    #[derive(Deserialize)]
    struct UploadResponse {
        id: uuid::Uuid,
    }

    let response_text = response
        .text()
        .await
        .context("Failed to read photo upload response body")?;

    let upload_response: UploadResponse =
        serde_json::from_str(&response_text).with_context(|| {
            format!(
                "Failed to parse photo upload response as JSON: {}",
                response_text
            )
        })?;

    Ok(upload_response.id)
}

/// Convert a Paprika recipe to RawRecipe format for the import endpoint
fn convert_to_raw_recipe(recipe: &PaprikaRecipe, preserve_tags: bool) -> ImportRawRecipe {
    ImportRawRecipe {
        title: recipe.name.clone(),
        description: recipe.description.clone(),
        // Keep ingredients as raw newline-separated string - pipeline will parse
        ingredients: recipe.ingredients.clone().unwrap_or_default(),
        instructions: recipe.directions.clone().unwrap_or_default(),
        image_urls: vec![], // Photos are uploaded separately
        source_url: recipe.source_url.clone(),
        source_name: recipe.source.clone(),
        servings: recipe.servings.clone(),
        prep_time: recipe.prep_time.clone(),
        cook_time: recipe.cook_time.clone(),
        total_time: recipe.total_time.clone(),
        rating: recipe.rating,
        difficulty: recipe.difficulty.clone(),
        nutritional_info: recipe.nutritional_info.clone(),
        notes: recipe.notes.clone(),
        categories: if preserve_tags {
            recipe.categories.clone()
        } else {
            None
        },
    }
}

/// Call the import endpoint
async fn import_recipe(
    config: &Configuration,
    raw_recipe: ImportRawRecipe,
    photo_ids: Vec<uuid::Uuid>,
) -> Result<ImportRecipeResponse> {
    let client = reqwest::Client::new();

    let request_body = ImportRecipeRequest {
        raw_recipe,
        photo_ids,
        extraction_method: "paprika".to_string(),
    };

    let mut request = client
        .post(format!("{}/api/import/recipe", config.base_path))
        .json(&request_body);

    if let Some(ref token) = config.bearer_access_token {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .await
        .context("Failed to send import request")?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "Import failed with status {} ({}): {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or("Unknown"),
            body
        );
    }

    let response_text = response
        .text()
        .await
        .context("Failed to read import response body")?;

    let import_response: ImportRecipeResponse = serde_json::from_str(&response_text)
        .with_context(|| format!("Failed to parse import response as JSON: {}", response_text))?;

    Ok(import_response)
}

pub async fn import(
    server: &str,
    username: &str,
    password: &str,
    preserve_tags: bool,
    file_path: &Path,
) -> Result<()> {
    // Authenticate
    let mut config = Configuration::new();
    config.base_path = server.to_string();

    let login_response = auth_api::login(
        &config,
        LoginRequest {
            username: username.to_string(),
            password: password.to_string(),
        },
    )
    .await
    .context("Failed to login")?;

    config.bearer_access_token = Some(login_response.token);

    // Open the paprikarecipes file
    let file = File::open(file_path)
        .with_context(|| format!("Failed to open file: {}", file_path.display()))?;

    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("Failed to read zip archive: {}", file_path.display()))?;

    tracing::info!("Found {} recipes in archive", archive.len());

    let mut jobs = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let entry_name = entry.name().to_string();

        if !entry_name.ends_with(".paprikarecipe") {
            tracing::debug!(file = %entry_name, "Skipping non-recipe file");
            continue;
        }

        // Read the gzipped content
        let mut compressed_data = Vec::new();
        entry.read_to_end(&mut compressed_data)?;

        // Decompress with gzip
        let mut decoder = GzDecoder::new(&compressed_data[..]);
        let mut json_content = String::new();
        decoder
            .read_to_string(&mut json_content)
            .with_context(|| format!("Failed to decompress recipe: {}", entry_name))?;

        // Parse the recipe JSON
        let recipe: PaprikaRecipe = serde_json::from_str(&json_content)
            .with_context(|| format!("Failed to parse recipe JSON: {}", entry_name))?;

        let recipe_name = recipe.name.clone();

        // Upload all photos from the photos array (these are full resolution)
        // Fall back to photo_data if photos array is empty
        let mut photo_ids = Vec::new();
        if !recipe.photos.is_empty() {
            for (i, photo) in recipe.photos.iter().enumerate() {
                if let Some(ref data) = photo.data {
                    if !data.is_empty() {
                        match base64::engine::general_purpose::STANDARD.decode(data) {
                            Ok(image_bytes) => match upload_photo(&config, &image_bytes).await {
                                Ok(id) => photo_ids.push(id),
                                Err(e) => {
                                    tracing::warn!(
                                        photo_num = i + 1,
                                        recipe = %recipe_name,
                                        error = %e,
                                        "Failed to upload photo"
                                    );
                                }
                            },
                            Err(e) => {
                                tracing::warn!(
                                    photo_num = i + 1,
                                    recipe = %recipe_name,
                                    error = %e,
                                    "Failed to decode photo"
                                );
                            }
                        }
                    }
                }
            }
        } else if let Some(ref data) = recipe.photo_data {
            // Fall back to photo_data (may be a thumbnail, but better than nothing)
            if !data.is_empty() {
                match base64::engine::general_purpose::STANDARD.decode(data) {
                    Ok(image_bytes) => match upload_photo(&config, &image_bytes).await {
                        Ok(id) => photo_ids.push(id),
                        Err(e) => {
                            tracing::warn!(
                                recipe = %recipe_name,
                                error = %e,
                                "Failed to upload photo"
                            );
                        }
                    },
                    Err(e) => {
                        tracing::warn!(
                            recipe = %recipe_name,
                            error = %e,
                            "Failed to decode photo"
                        );
                    }
                }
            }
        }

        // Convert to RawRecipe format and call the import endpoint
        let raw_recipe = convert_to_raw_recipe(&recipe, preserve_tags);

        let response = import_recipe(&config, raw_recipe, photo_ids)
            .await
            .with_context(|| format!("Failed to submit recipe '{recipe_name}'"))?;
        tracing::info!(
            "  Submitted: {} (job_id: {}, status: {})",
            recipe_name,
            response.job_id,
            response.status
        );
        jobs.push(response.job_id);
    }

    let saved_count = jobs.len();
    let enrichment_failures =
        tokio::time::timeout(Duration::from_secs(600), wait_for_imports(&config, jobs))
            .await
            .context(
                "Timed out waiting for import jobs; submitted recipes remain on the server",
            )??;

    tracing::info!("IMPORT COMPLETE");
    tracing::info!("Recipes saved: {}", saved_count);
    tracing::info!("Recipes with enrichment failures: {}", enrichment_failures);

    Ok(())
}

/// Wait for the whole batch without serializing the server's background work.
/// Completed jobs can still have failed optional enrichment steps.
async fn wait_for_imports(config: &Configuration, mut jobs: Vec<uuid::Uuid>) -> Result<usize> {
    let mut enrichment_failures = 0;
    while !jobs.is_empty() {
        let mut pending = Vec::new();
        for id in jobs {
            let job = scrape_api::get_scrape(config, &id.to_string())
                .await
                .with_context(|| format!("Failed to read import job {id}"))?;
            match job.status.as_str() {
                "completed" | "failed" => {
                    let saved = job
                        .steps
                        .iter()
                        .any(|step| step.name == "save_recipe" && step.status == "completed");
                    let enrichment_failed = job
                        .failed_at_step
                        .as_ref()
                        .and_then(|step| step.as_deref())
                        .is_some_and(|step| step.starts_with("enrich_"));
                    if job.status == "failed" && !(saved && enrichment_failed) {
                        bail!("Import job {id} failed; inspect its status for details; saved data is preserved");
                    }
                    if job.status == "completed" && job.recipe_id.flatten().is_none() {
                        bail!("Import job {id} completed without a saved recipe");
                    }
                    let failed_steps: Vec<_> = job
                        .steps
                        .iter()
                        .filter(|step| step.status == "failed")
                        .map(|step| step.name.as_str())
                        .collect();
                    if !failed_steps.is_empty() {
                        enrichment_failures += 1;
                        // Report step names, not provider response bodies or credentials.
                        tracing::warn!(job_id = %id, steps = %failed_steps.join(", "),
                            "Recipe saved, but enrichment failed; inspect the import job for details");
                    }
                }
                "pending" | "scraping" | "parsing" => pending.push(id),
                status => bail!("Import job {id} has unexpected status: {status}"),
            }
        }
        jobs = pending;
        if !jobs.is_empty() {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
    Ok(enrichment_failures)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::State, routing::get, Json, Router};
    use ramekin_client::models::{ScrapeJobResponse, StepState};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    async fn job_status(
        State(responses): State<Arc<Mutex<VecDeque<ScrapeJobResponse>>>>,
    ) -> Json<ScrapeJobResponse> {
        Json(
            responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected poll"),
        )
    }

    async fn poll_responses(responses: Vec<ScrapeJobResponse>) -> Result<usize> {
        let mut jobs = Vec::new();
        for response in &responses {
            if !jobs.contains(&response.id) {
                jobs.push(response.id);
            }
        }
        let responses = Arc::new(Mutex::new(VecDeque::from(responses)));
        let app = Router::new()
            .route("/api/scrape/{id}", get(job_status))
            .with_state(responses.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut config = Configuration::new();
        config.base_path = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let result = wait_for_imports(&config, jobs).await;
        server.abort();
        assert!(responses.lock().unwrap().is_empty());
        result
    }

    fn job(status: &str, saved: bool, failed_step: Option<&str>) -> ScrapeJobResponse {
        let mut job = ScrapeJobResponse {
            id: uuid::Uuid::new_v4(),
            created_at: "2026-09-09T00:00:00Z".into(),
            status: status.into(),
            ..Default::default()
        };
        if saved {
            job.steps.push(StepState::new(
                true,
                "save_recipe".into(),
                "completed".into(),
            ));
            if status == "completed" {
                job.recipe_id = Some(Some(uuid::Uuid::new_v4()));
            }
        }
        if let Some(step) = failed_step {
            job.failed_at_step = Some(Some(step.into()));
            job.steps
                .push(StepState::new(true, step.into(), "failed".into()));
        }
        job
    }

    #[tokio::test]
    async fn waits_for_terminal_status() {
        let pending = job("pending", false, None);
        let mut completed = job("completed", true, None);
        completed.id = pending.id;
        assert_eq!(poll_responses(vec![pending, completed]).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn saved_recipe_with_failed_enrichment_is_counted() {
        for status in ["completed", "failed"] {
            assert_eq!(
                poll_responses(vec![job(status, true, Some("enrich_auto_tag"))])
                    .await
                    .unwrap(),
                1
            );
        }
    }

    #[tokio::test]
    async fn batch_waits_for_remaining_jobs_without_recounting_finished_ones() {
        let failed = job("failed", true, Some("enrich_auto_tag"));
        let pending = job("parsing", false, None);
        let mut completed = job("completed", true, None);
        completed.id = pending.id;
        assert_eq!(
            poll_responses(vec![failed, pending, completed]).await.unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn failures_before_save_and_apply_failures_are_errors() {
        for (saved, step) in [
            (false, "parse_ingredients"),
            (true, "apply_auto_tags"),
            (false, "enrich_auto_tag"),
        ] {
            let error = poll_responses(vec![job("failed", saved, Some(step))])
                .await
                .unwrap_err();
            assert!(error.to_string().contains("failed; inspect its status"));
        }
    }

    #[tokio::test]
    async fn completed_without_recipe_and_unknown_status_are_errors() {
        for status in ["completed", "unknown"] {
            assert!(poll_responses(vec![job(status, false, None)])
                .await
                .is_err());
        }
    }
}
