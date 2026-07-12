use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
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
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;

    let (current_version_id, source_url) = fetch_rescrape_target(&pool, user_id, recipe_id).await?;

    // Create rescrape job with recipe_id pre-populated.
    let job =
        scraping::create_rescrape_job(&pool, user_id, recipe_id, current_version_id, &source_url)
            .await
            .map_err(|e| {
                tracing::error!("Failed to create rescrape job: {}", e);
                ApiError::internal("Failed to create rescrape job")
            })?;

    // Spawn background task
    scraping::spawn_scrape_job(pool.clone(), job.id, &source_url, "rescrape");

    Ok((
        StatusCode::CREATED,
        Json(RescrapeResponse {
            job_id: job.id,
            status: job.status,
        }),
    ))
}

/// Look up the recipe's current version id and validated rescrape source URL:
/// the recipe must belong to the user, have a current version, and carry a
/// non-empty source URL on an allowed host.
pub(super) async fn fetch_rescrape_target(
    pool: &DbPool,
    user_id: Uuid,
    recipe_id: Uuid,
) -> Result<(Uuid, String), ApiError> {
    let (current_version_id, source_url) = run_db(pool, move |conn| {
        // Verify the recipe exists and belongs to the user
        let current_version_id: Option<Uuid> = recipes::table
            .filter(recipes::id.eq(recipe_id))
            .filter(recipes::user_id.eq(user_id))
            .filter(recipes::deleted_at.is_null())
            .select(recipes::current_version_id)
            .first(conn)
            .map_err(|e| match e {
                diesel::NotFound => ApiError::not_found("Recipe not found"),
                _ => ApiError::internal("Failed to fetch recipe"),
            })?;

        // Get current version to extract source_url
        let current_version_id = match current_version_id {
            Some(vid) => vid,
            None => return Err(ApiError::invalid_request("Recipe has no versions")),
        };

        let source_url: Option<String> = recipe_versions::table
            .filter(recipe_versions::id.eq(current_version_id))
            .select(recipe_versions::source_url)
            .first(conn)
            .map_err(|_| ApiError::internal("Failed to fetch recipe version"))?;

        Ok((current_version_id, source_url))
    })
    .await?;

    // Require source_url for rescrape
    let source_url = match source_url {
        Some(url) if !url.is_empty() => url,
        _ => {
            return Err(ApiError::invalid_request(
                "Recipe has no source URL to rescrape from",
            ))
        }
    };

    // Check if host is allowed
    if let Err(e) = scraping::is_host_allowed(&source_url) {
        return Err(ApiError::invalid_request(e.to_string()));
    }

    Ok((current_version_id, source_url))
}
