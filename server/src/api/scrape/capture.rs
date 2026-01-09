use crate::api::scrape::create::CreateScrapeResponse;
use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::scraping;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CaptureRequest {
    /// The HTML content of the page to extract a recipe from
    pub html: String,
    /// The URL the HTML came from (used for source attribution)
    pub source_url: String,
}

#[utoipa::path(
    post,
    path = "/api/scrape/capture",
    tag = "scrape",
    request_body = CaptureRequest,
    responses(
        (status = 201, description = "Scrape job created from captured HTML", body = CreateScrapeResponse),
        (status = 400, description = "Invalid URL", body = ErrorResponse),
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

    // Create job with pre-existing HTML
    let job =
        match scraping::create_job_with_html(&pool, user.id, &request.source_url, &request.html) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!("Failed to create capture job: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to create capture job".to_string(),
                    }),
                )
                    .into_response();
            }
        };

    let job_id = job.id;

    // Spawn background task to process from extract_recipe step
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        scraping::run_scrape_job(pool_clone, job_id).await;
    });

    tracing::info!(
        "Created capture job {} for URL {}",
        job.id,
        request.source_url
    );

    (
        StatusCode::CREATED,
        Json(CreateScrapeResponse {
            id: job.id,
            status: job.status,
        }),
    )
        .into_response()
}
