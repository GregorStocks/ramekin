use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::scraping;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RetryScrapeResponse {
    /// The scrape job ID
    pub id: Uuid,
    /// New job status after retry
    pub status: String,
}

#[utoipa::path(
    post,
    path = "/api/scrape/{id}/retry",
    tag = "scrape",
    params(
        ("id" = Uuid, Path, description = "Scrape job ID")
    ),
    responses(
        (status = 200, description = "Retry initiated", body = RetryScrapeResponse),
        (status = 400, description = "Cannot retry job", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Job not found", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn retry_scrape(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(job_id): Path<Uuid>,
) -> impl IntoResponse {
    // Get job to check ownership
    let job = match scraping::get_job(&pool, job_id) {
        Ok(j) => j,
        Err(scraping::ScrapeError::JobNotFound) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Scrape job not found".to_string(),
                }),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get scrape job: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to get scrape job".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Check ownership
    if job.user_id != user.id {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Scrape job not found".to_string(),
            }),
        )
            .into_response();
    }

    // Retry job
    let new_status = match scraping::retry_job(&pool, job_id) {
        Ok(s) => s,
        Err(scraping::ScrapeError::InvalidState(msg)) => {
            return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: msg })).into_response();
        }
        Err(e) => {
            tracing::error!("Failed to retry scrape job: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to retry scrape job".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Spawn background task to continue processing
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        scraping::run_scrape_job(pool_clone, job_id).await;
    });

    (
        StatusCode::OK,
        Json(RetryScrapeResponse {
            id: job_id,
            status: new_status,
        }),
    )
        .into_response()
}
