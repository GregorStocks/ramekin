use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::scraping;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use ramekin_core::{ExtractionMethod, RawRecipe};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

/// Extraction method for imported recipes (mirrors ramekin_core::ExtractionMethod)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportExtractionMethod {
    JsonLd,
    Microdata,
    Paprika,
    PhotoUpload,
}

impl From<ImportExtractionMethod> for ExtractionMethod {
    fn from(method: ImportExtractionMethod) -> Self {
        match method {
            ImportExtractionMethod::JsonLd => ExtractionMethod::JsonLd,
            ImportExtractionMethod::Microdata => ExtractionMethod::Microdata,
            ImportExtractionMethod::Paprika => ExtractionMethod::Paprika,
            ImportExtractionMethod::PhotoUpload => ExtractionMethod::PhotoUpload,
        }
    }
}

/// Raw recipe data for import (mirrors ramekin_core::RawRecipe)
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ImportRawRecipe {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Ingredients as a newline-separated blob
    pub ingredients: String,
    /// Instructions as a blob (could be HTML or plain text)
    pub instructions: String,
    /// Image URLs found in the recipe (not used for imports with pre-uploaded photos)
    #[serde(default)]
    pub image_urls: Vec<String>,
    /// Source URL (optional for imports without a web source)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servings: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prep_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cook_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nutritional_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
}

impl From<ImportRawRecipe> for RawRecipe {
    fn from(recipe: ImportRawRecipe) -> Self {
        RawRecipe {
            title: recipe.title,
            description: recipe.description,
            ingredients: recipe.ingredients,
            instructions: recipe.instructions,
            image_urls: recipe.image_urls,
            source_url: recipe.source_url,
            source_name: recipe.source_name,
            servings: recipe.servings,
            prep_time: recipe.prep_time,
            cook_time: recipe.cook_time,
            total_time: recipe.total_time,
            rating: recipe.rating,
            difficulty: recipe.difficulty,
            nutritional_info: recipe.nutritional_info,
            notes: recipe.notes,
            categories: recipe.categories,
            footnotes: None,
        }
    }
}

/// Request body for importing a recipe
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ImportRecipeRequest {
    /// The raw recipe data (converted from import source by client)
    pub raw_recipe: ImportRawRecipe,
    /// Photo IDs that have already been uploaded via POST /api/photos
    pub photo_ids: Vec<Uuid>,
    /// The extraction/import method used
    pub extraction_method: ImportExtractionMethod,
    /// Client-chosen key for this import. Resubmitting a key the user already
    /// used returns the original job (200) instead of creating another recipe;
    /// the resubmission's photo_ids are then ignored.
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

const MAX_IDEMPOTENCY_KEY_LEN: usize = 255;
const MAX_LOOKUP_KEYS: usize = 1000;

fn validate_key(key: &str) -> Result<(), ApiError> {
    if key.is_empty() || key.len() > MAX_IDEMPOTENCY_KEY_LEN {
        return Err(ApiError::invalid_request(format!(
            "idempotency_key must be 1-{MAX_IDEMPOTENCY_KEY_LEN} bytes"
        )));
    }
    Ok(())
}

/// Response from recipe import
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ImportRecipeResponse {
    /// The created job ID
    pub job_id: Uuid,
    /// Current job status
    pub status: String,
}

#[utoipa::path(
    post,
    path = "/api/import/recipe",
    tag = "import",
    request_body = ImportRecipeRequest,
    responses(
        (status = 200, description = "Existing job for a repeated idempotency_key", body = ImportRecipeResponse),
        (status = 201, description = "Import job created", body = ImportRecipeResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn import_recipe(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<ImportRecipeRequest>,
) -> impl IntoResponse {
    if let Some(key) = request.idempotency_key.as_deref() {
        if let Err(e) = validate_key(key) {
            return e.into_response();
        }
    }
    let raw_recipe: RawRecipe = request.raw_recipe.into();
    let extraction_method: ExtractionMethod = request.extraction_method.into();
    let source_url = raw_recipe.source_url.clone();
    let title = raw_recipe.title.clone();

    // Create import job with pre-populated step outputs
    let (job, created) = match scraping::create_import_job(
        &pool,
        user.id,
        source_url.as_deref(),
        raw_recipe,
        extraction_method,
        request.photo_ids,
        request.idempotency_key,
    )
    .await
    {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("Failed to create import job: {}", e);
            return ApiError::internal(format!("Failed to create import job: {}", e))
                .into_response();
        }
    };

    let status = if created {
        tracing::info!("Created import job {} for recipe '{}'", job.id, title);
        // Spawn background task to run the pipeline
        scraping::spawn_import_job(pool.clone(), job.id);
        StatusCode::CREATED
    } else {
        tracing::info!("Import of '{}' repeats job {}", title, job.id);
        StatusCode::OK
    };

    (
        status,
        Json(ImportRecipeResponse {
            job_id: job.id,
            status: job.status,
        }),
    )
        .into_response()
}

/// Request body for looking up import jobs by idempotency key
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct LookupImportJobsRequest {
    /// Keys previously sent as ImportRecipeRequest.idempotency_key (at most 1000)
    pub idempotency_keys: Vec<String>,
}

/// An import job submitted with an idempotency key
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ImportJobKey {
    pub idempotency_key: String,
    pub job_id: Uuid,
}

/// The caller's import jobs for the requested keys; unknown keys are omitted
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LookupImportJobsResponse {
    pub jobs: Vec<ImportJobKey>,
}

#[utoipa::path(
    post,
    path = "/api/import/recipe/lookup",
    tag = "import",
    request_body = LookupImportJobsRequest,
    responses(
        (status = 200, description = "Jobs for the keys this user has used", body = LookupImportJobsResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn lookup_import_jobs(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<LookupImportJobsRequest>,
) -> impl IntoResponse {
    if request.idempotency_keys.len() > MAX_LOOKUP_KEYS {
        return ApiError::invalid_request(format!("At most {MAX_LOOKUP_KEYS} keys per lookup"))
            .into_response();
    }
    for key in &request.idempotency_keys {
        if let Err(e) = validate_key(key) {
            return e.into_response();
        }
    }
    match scraping::find_import_jobs_by_key(&pool, user.id, request.idempotency_keys).await {
        Ok(jobs) => Json(LookupImportJobsResponse {
            jobs: jobs
                .into_iter()
                .map(|(idempotency_key, job_id)| ImportJobKey {
                    idempotency_key,
                    job_id,
                })
                .collect(),
        })
        .into_response(),
        Err(e) => {
            tracing::error!("Failed to look up import jobs: {}", e);
            ApiError::internal(format!("Failed to look up import jobs: {}", e)).into_response()
        }
    }
}
