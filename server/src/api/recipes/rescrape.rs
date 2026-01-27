use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::schema::{recipe_versions, recipes};
use crate::scraping;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RescrapeResponse {
    /// The scrape job ID
    pub job_id: Uuid,
    /// Current job status
    pub status: String,
}

#[utoipa::path(
    post,
    path = "/api/recipes/{id}/rescrape",
    tag = "recipes",
    params(
        ("id" = Uuid, Path, description = "Recipe ID")
    ),
    responses(
        (status = 201, description = "Rescrape job created", body = RescrapeResponse),
        (status = 400, description = "Recipe has no source URL", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Recipe not found", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn rescrape(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(recipe_id): Path<Uuid>,
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    // Verify the recipe exists and belongs to the user
    let recipe: (Uuid, Option<Uuid>) = match recipes::table
        .filter(recipes::id.eq(recipe_id))
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .select((recipes::id, recipes::current_version_id))
        .first(&mut conn)
    {
        Ok(r) => r,
        Err(diesel::NotFound) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Recipe not found".to_string(),
                }),
            )
                .into_response()
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch recipe".to_string(),
                }),
            )
                .into_response()
        }
    };

    let (recipe_id, current_version_id) = recipe;

    // Get current version to extract source_url
    let current_version_id = match current_version_id {
        Some(vid) => vid,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Recipe has no versions".to_string(),
                }),
            )
                .into_response()
        }
    };

    let source_url: Option<String> = match recipe_versions::table
        .filter(recipe_versions::id.eq(current_version_id))
        .select(recipe_versions::source_url)
        .first(&mut conn)
    {
        Ok(url) => url,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch recipe version".to_string(),
                }),
            )
                .into_response()
        }
    };

    // Require source_url for rescrape
    let source_url = match source_url {
        Some(url) if !url.is_empty() => url,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Recipe has no source URL to rescrape from".to_string(),
                }),
            )
                .into_response()
        }
    };

    // Check if host is allowed
    if let Err(e) = scraping::is_host_allowed(&source_url) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response();
    }

    // Create rescrape job with recipe_id pre-populated
    let job = match scraping::create_rescrape_job(&pool, user.id, recipe_id, &source_url) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("Failed to create rescrape job: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to create rescrape job".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Spawn background task
    scraping::spawn_scrape_job(pool.clone(), job.id, &source_url, "rescrape");

    (
        StatusCode::CREATED,
        Json(RescrapeResponse {
            job_id: job.id,
            status: job.status,
        }),
    )
        .into_response()
}
