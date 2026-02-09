use crate::api::ErrorResponse;
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
    let raw_recipe: RawRecipe = request.raw_recipe.into();
    let extraction_method: ExtractionMethod = request.extraction_method.into();

    // Create import job with pre-populated step outputs
    let job = match scraping::create_import_job(
        &pool,
        user.id,
        raw_recipe.source_url.as_deref(),
        &raw_recipe,
        extraction_method,
        request.photo_ids,
    ) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("Failed to create import job: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to create import job: {}", e),
                }),
            )
                .into_response();
        }
    };

    tracing::info!(
        "Created import job {} for recipe '{}'",
        job.id,
        raw_recipe.title
    );

    // Spawn background task to run the pipeline
    scraping::spawn_import_job(pool.clone(), job.id);

    (
        StatusCode::CREATED,
        Json(ImportRecipeResponse {
            job_id: job.id,
            status: job.status,
        }),
    )
        .into_response()
}
