use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::scraping;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CaptureRequest {
    /// The HTML content of the page to extract a recipe from
    pub html: String,
    /// The URL the HTML came from (used for source attribution)
    pub source_url: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CaptureResponse {
    /// The created recipe ID
    pub recipe_id: Uuid,
    /// The extracted recipe title
    pub title: String,
}

#[utoipa::path(
    post,
    path = "/api/scrape/capture",
    tag = "scrape",
    request_body = CaptureRequest,
    responses(
        (status = 201, description = "Recipe created from captured HTML", body = CaptureResponse),
        (status = 400, description = "Invalid URL or no recipe found", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn capture(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<CaptureRequest>,
) -> impl IntoResponse {
    // Validate URL format
    if let Err(e) = reqwest::Url::parse(&request.source_url) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid URL: {}", e),
            }),
        )
            .into_response();
    }

    // Extract recipe from HTML
    let raw_recipe = match ramekin_core::extract_recipe(&request.html, &request.source_url) {
        Ok(recipe) => recipe,
        Err(e) => {
            tracing::warn!("Failed to extract recipe: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Could not find recipe in page: {}", e),
                }),
            )
                .into_response();
        }
    };

    let title = raw_recipe.title.clone();

    // Create recipe in database
    match scraping::create_recipe_from_raw(&pool, user.id, &raw_recipe) {
        Ok(recipe_id) => {
            tracing::info!("Created recipe {} from captured HTML", recipe_id);
            (
                StatusCode::CREATED,
                Json(CaptureResponse { recipe_id, title }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to create recipe: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to save recipe".to_string(),
                }),
            )
                .into_response()
        }
    }
}
