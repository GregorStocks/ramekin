use crate::api::recipes::rescrape::{fetch_rescrape_target, RescrapeResponse};
use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::scraping;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

#[utoipa::path(
    post,
    path = "/api/recipes/{id}/rescrape-photo",
    tag = "recipes",
    params(
        ("id" = Uuid, Path, description = "Recipe ID")
    ),
    responses(
        (status = 201, description = "Photo rescrape job created", body = RescrapeResponse),
        (status = 400, description = "Recipe has no source URL", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Recipe not found", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn rescrape_photo(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(recipe_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;

    let (current_version_id, source_url) = fetch_rescrape_target(&pool, user_id, recipe_id).await?;

    let job = scraping::create_photo_rescrape_job(
        &pool,
        user_id,
        recipe_id,
        current_version_id,
        &source_url,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to create photo rescrape job: {}", e);
        ApiError::internal("Failed to create photo rescrape job")
    })?;

    scraping::spawn_scrape_job(pool.clone(), job.id, &source_url, "rescrape_photo");

    Ok((
        StatusCode::CREATED,
        Json(RescrapeResponse {
            job_id: job.id,
            status: job.status,
        }),
    ))
}
