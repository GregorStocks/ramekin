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
pub struct ScrapeJobResponse {
    /// The scrape job ID
    pub id: Uuid,
    /// Current job status (pending, scraping, parsing, completed, failed)
    pub status: String,
    /// URL being scraped
    pub url: String,
    /// Recipe ID if completed successfully
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipe_id: Option<Uuid>,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Which step failed (for retry logic)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_at_step: Option<String>,
    /// Whether this job can be retried
    pub can_retry: bool,
    /// Number of retry attempts
    pub retry_count: i32,
}

#[utoipa::path(
    get,
    path = "/api/scrape/{id}",
    tag = "scrape",
    params(
        ("id" = Uuid, Path, description = "Scrape job ID")
    ),
    responses(
        (status = 200, description = "Scrape job status", body = ScrapeJobResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Job not found", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_scrape(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(job_id): Path<Uuid>,
) -> impl IntoResponse {
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

    let can_retry = job.status == scraping::STATUS_FAILED;

    (
        StatusCode::OK,
        Json(ScrapeJobResponse {
            id: job.id,
            status: job.status,
            url: job.url,
            recipe_id: job.recipe_id,
            error: job.error_message,
            failed_at_step: job.failed_at_step,
            can_retry,
            retry_count: job.retry_count,
        }),
    )
        .into_response()
}
