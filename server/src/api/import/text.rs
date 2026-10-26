use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::ids::Uuid;
use crate::scraping;

#[derive(Deserialize, ToSchema)]
pub struct ImportFromTextRequest {
    /// A whole recipe, typed or pasted as unstructured text.
    pub text: String,
}

#[derive(Serialize, ToSchema)]
pub struct ImportFromTextResponse {
    /// The created job ID
    pub job_id: Uuid,
    /// Current job status
    pub status: String,
}

#[utoipa::path(
    post, path = "/api/import/text", tag = "import",
    request_body = ImportFromTextRequest,
    responses(
        (status = 201, description = "Text import job created", body = ImportFromTextResponse),
        (status = 400, description = "Invalid recipe text", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn import_from_text(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<ImportFromTextRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if request.text.trim().is_empty() || request.text.len() > 50_000 {
        return Err(ApiError::invalid_request(
            "Enter recipe text between 1 and 50,000 bytes.",
        ));
    }
    let job = scraping::create_text_import_job(&pool, user.id, &request.text)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create text import job: {}", e);
            ApiError::internal("Failed to create import job")
        })?;
    scraping::spawn_import_job(pool.clone(), job.id, "text_import");
    Ok((
        StatusCode::CREATED,
        Json(ImportFromTextResponse {
            job_id: job.id,
            status: job.status,
        }),
    ))
}
