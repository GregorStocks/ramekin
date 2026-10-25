//! Edit with AI: start a job that edits a recipe the way the user describes
//! and proposes the result as a new version (reviewed and accepted with the
//! normalize proposal endpoints).

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::api::recipes::normalize::{NormalizeJobResponse, RecipeSource};
use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::ids::Uuid;
use crate::photos::{load_photo_images, PhotoImageLoadError};
use crate::scraping::{self, EditRequest};

/// Request body for Edit with AI.
#[derive(Debug, Deserialize, ToSchema)]
pub struct EditRecipeRequest {
    /// The change the user wants, in their words.
    pub instruction: String,
    /// Photos of the dish to give the model as context.
    #[serde(default)]
    pub photo_ids: Vec<Uuid>,
}

/// Edit a recipe with AI
///
/// Starts a job that edits the recipe's current version the way the
/// instruction says, looking up catalog weights and calories and the user's
/// tags and recipes as it goes. Poll it with `GET /api/scrape/{id}`; once it
/// completes, `GET /api/recipes/{id}/normalize/{job_id}` returns the proposed
/// version and a summary of the changes, and
/// `POST /api/recipes/{id}/normalize/{job_id}/accept` saves it.
#[utoipa::path(
    post,
    path = "/api/recipes/{id}/edit-with-ai",
    tag = "edit_recipe",
    params(("id" = Uuid, Path, description = "Recipe ID")),
    request_body = EditRecipeRequest,
    responses(
        (status = 201, description = "Edit job started; it proposes a version", body = NormalizeJobResponse),
        (status = 400, description = "Empty instruction, or invalid photo IDs", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Recipe not found", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn edit_recipe_with_ai(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(recipe_id): Path<Uuid>,
    Json(request): Json<EditRecipeRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let instruction = request.instruction.trim();
    if instruction.is_empty() {
        return Err(ApiError::invalid_request("Say what to change"));
    }
    let source = RecipeSource::load(&pool, user.id, recipe_id).await?;
    // Check the photos now, so a bad id fails the request rather than the job.
    match load_photo_images(&pool, user.id, &request.photo_ids).await {
        Ok(_) => {}
        Err(PhotoImageLoadError::NotFound) => {
            return Err(ApiError::invalid_request(
                "One or more photo_ids not found or don't belong to user",
            ))
        }
        Err(e @ (PhotoImageLoadError::Database(_) | PhotoImageLoadError::File(_))) => {
            tracing::error!("Failed to load photos for Edit with AI: {e}");
            return Err(ApiError::internal("Failed to load photos"));
        }
    }

    let edit = EditRequest {
        instruction: instruction.to_string(),
        photo_ids: request.photo_ids,
    };
    let job =
        scraping::create_edit_job(&pool, user.id, recipe_id, source.current_version_id, &edit)
            .await
            .map_err(|e| {
                tracing::error!("Failed to create Edit with AI job: {e}");
                ApiError::internal("Failed to start editing")
            })?;
    scraping::spawn_import_job(pool.clone(), job.id, "edit_with_ai");
    Ok((
        StatusCode::CREATED,
        Json(NormalizeJobResponse {
            job_id: job.id,
            status: job.status,
        }),
    ))
}
