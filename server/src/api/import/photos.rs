use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::schema::photos;
use crate::scraping;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ImportFromPhotosRequest {
    /// Photo IDs that have already been uploaded via POST /api/photos
    pub photo_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ImportFromPhotosResponse {
    /// The created job ID
    pub job_id: Uuid,
    /// Current job status
    pub status: String,
}

#[utoipa::path(
    post,
    path = "/api/import/photos",
    tag = "import",
    request_body = ImportFromPhotosRequest,
    responses(
        (status = 201, description = "Photo import job created", body = ImportFromPhotosResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn import_from_photos(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<ImportFromPhotosRequest>,
) -> impl IntoResponse {
    // Validate: must have at least one photo
    if request.photo_ids.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "At least one photo_id is required".to_string(),
            }),
        )
            .into_response();
    }

    // Verify all photos exist and belong to this user
    {
        let mut conn = match pool.get() {
            Ok(c) => c,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Database error: {}", e),
                    }),
                )
                    .into_response();
            }
        };

        let found_count: i64 = match photos::table
            .filter(photos::id.eq_any(&request.photo_ids))
            .filter(photos::user_id.eq(user.id))
            .filter(photos::deleted_at.is_null())
            .count()
            .get_result(&mut conn)
        {
            Ok(count) => count,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Database error: {}", e),
                    }),
                )
                    .into_response();
            }
        };

        if found_count != request.photo_ids.len() as i64 {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "One or more photo_ids not found or don't belong to user".to_string(),
                }),
            )
                .into_response();
        }
    }

    // Create a pending scrape job (no URL for photo imports)
    let job = match scraping::create_pending_photo_job(&pool, user.id) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("Failed to create photo import job: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to create import job".to_string(),
                }),
            )
                .into_response();
        }
    };

    tracing::info!(
        "Created photo import job {} with {} photos",
        job.id,
        request.photo_ids.len()
    );

    // Spawn background task
    scraping::spawn_photo_import_job(pool.clone(), job.id, user.id, request.photo_ids);

    (
        StatusCode::CREATED,
        Json(ImportFromPhotosResponse {
            job_id: job.id,
            status: job.status,
        }),
    )
        .into_response()
}
