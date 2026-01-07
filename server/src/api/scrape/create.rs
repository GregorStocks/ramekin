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
pub struct CreateScrapeRequest {
    /// URL to scrape for recipe data
    pub url: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CreateScrapeResponse {
    /// The scrape job ID
    pub id: Uuid,
    /// Current job status
    pub status: String,
}

#[utoipa::path(
    post,
    path = "/api/scrape",
    tag = "scrape",
    request_body = CreateScrapeRequest,
    responses(
        (status = 201, description = "Scrape job created", body = CreateScrapeResponse),
        (status = 400, description = "Invalid URL", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_scrape(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<CreateScrapeRequest>,
) -> impl IntoResponse {
    // Validate URL format
    if let Err(e) = reqwest::Url::parse(&request.url) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid URL: {}", e),
            }),
        )
            .into_response();
    }

    // Check if host is allowed (for early failure)
    if let Err(e) = scraping::is_host_allowed(&request.url) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response();
    }

    // Create job
    let job = match scraping::create_job(&pool, user.id, &request.url) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("Failed to create scrape job: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to create scrape job".to_string(),
                }),
            )
                .into_response();
        }
    };

    let job_id = job.id;

    // Spawn background task
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        scraping::run_scrape_job(pool_clone, job_id).await;
    });

    (
        StatusCode::CREATED,
        Json(CreateScrapeResponse {
            id: job.id,
            status: job.status,
        }),
    )
        .into_response()
}
