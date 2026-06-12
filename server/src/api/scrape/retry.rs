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
            return ApiError::not_found("Scrape job not found").into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get scrape job: {}", e);
            return ApiError::internal("Failed to get scrape job").into_response();
        }
    };

    // Check ownership
    if job.user_id != user.id {
        return ApiError::not_found("Scrape job not found").into_response();
    }

    // Retry only makes sense for jobs with a URL (scrape jobs, not imports)
    let url = match &job.url {
        Some(u) => u,
        None => {
            return ApiError::invalid_request("Cannot retry import jobs").into_response();
        }
    };

    // Retry job
    let new_status = match scraping::retry_job(&pool, job_id) {
        Ok(s) => s,
        Err(scraping::ScrapeError::InvalidState(msg)) => {
            return ApiError::invalid_request(msg).into_response();
        }
        Err(e) => {
            tracing::error!("Failed to retry scrape job: {}", e);
            return ApiError::internal("Failed to retry scrape job").into_response();
        }
    };

    // Spawn background task with instrumentation
    scraping::spawn_scrape_job(pool.clone(), job_id, url, "retry");

    (
        StatusCode::OK,
        Json(RetryScrapeResponse {
            id: job_id,
            status: new_status,
        }),
    )
        .into_response()
}
