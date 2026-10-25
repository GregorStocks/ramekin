use crate::api::ai::ai_config_from_env;
use crate::api::ApiError;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::photos::{load_photo_images, PhotoImageLoadError};
use crate::types::RecipeContent;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use ramekin_core::ai::{edit_recipe, CachingAiClient};
use serde::Deserialize;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa::ToSchema;

/// Request body for Edit with AI.
#[derive(Debug, Deserialize, ToSchema)]
pub struct EditRecipeRequest {
    pub recipe: RecipeContent,
    pub instruction: String,
    #[serde(default)]
    pub photo_ids: Vec<crate::ids::Uuid>,
}

/// Edit a recipe with AI
///
/// Takes a recipe and a free-text instruction describing the desired change.
/// Returns the complete modified recipe. Stateless - does NOT modify any database records.
#[utoipa::path(
    post,
    path = "/api/edit-recipe",
    tag = "edit_recipe",
    request_body = EditRecipeRequest,
    responses(
        (status = 200, description = "Modified recipe", body = RecipeContent),
        (status = 400, description = "Invalid photo IDs", body = crate::api::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::api::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::api::ErrorResponse),
        (status = 503, description = "AI service unavailable", body = crate::api::ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn edit_recipe_with_ai(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<EditRecipeRequest>,
) -> impl IntoResponse {
    let ai_client = match ai_config_from_env() {
        Ok(config) => CachingAiClient::new(config.for_recipe_edit()),
        Err(e) => return e.into_response(),
    };

    // Serialize the recipe to JSON for the prompt
    let recipe_json = match serde_json::to_string_pretty(&request.recipe) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("Failed to serialize recipe for AI: {}", e);
            return ApiError::internal("Failed to serialize recipe").into_response();
        }
    };

    let images = match load_photo_images(&pool, user.id, &request.photo_ids).await {
        Ok(images) => images,
        Err(PhotoImageLoadError::NotFound) => {
            return ApiError::invalid_request(
                "One or more photo_ids not found or don't belong to user",
            )
            .into_response();
        }
        Err(e @ (PhotoImageLoadError::Database(_) | PhotoImageLoadError::File(_))) => {
            tracing::error!("Failed to load photos for Edit with AI: {}", e);
            return ApiError::internal("Failed to load photos").into_response();
        }
    };

    let modified =
        match edit_recipe::<RecipeContent>(&ai_client, &recipe_json, &request.instruction, images)
            .await
        {
            Ok(r) => r.recipe,
            Err(e) => {
                tracing::warn!("Edit with AI call failed: {}", e);
                return ApiError::service_unavailable(format!("AI service error: {}", e))
                    .into_response();
            }
        };

    (StatusCode::OK, Json(modified)).into_response()
}

#[derive(OpenApi)]
#[openapi(paths(edit_recipe_with_ai), components(schemas(EditRecipeRequest)))]
pub struct ApiDoc;
