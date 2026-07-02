use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::NewClientLogUpload;
use crate::schema::client_log_uploads;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

/// Matches the iOS DebugLogger rotation cap; a full log file always fits.
pub const MAX_CONTENT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateClientLogRequest {
    pub platform: String,
    pub app_version: Option<String>,
    pub os_info: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CreateClientLogResponse {
    pub id: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/client-logs",
    tag = "client_logs",
    request_body = CreateClientLogRequest,
    responses(
        (status = 201, description = "Log upload stored", body = CreateClientLogResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 413, description = "Content too large", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_client_log(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<CreateClientLogRequest>,
) -> impl IntoResponse {
    if request.platform != "ios" && request.platform != "web" {
        return ApiError::invalid_request("platform must be \"ios\" or \"web\"").into_response();
    }
    if request.content.is_empty() {
        return ApiError::invalid_request("content must not be empty").into_response();
    }
    if request.content.len() > MAX_CONTENT_BYTES {
        return ApiError::payload_too_large(format!(
            "content exceeds maximum size of {MAX_CONTENT_BYTES} bytes"
        ))
        .into_response();
    }

    let mut conn = get_conn!(pool);
    let new_upload = NewClientLogUpload {
        user_id: user.id,
        platform: &request.platform,
        app_version: request.app_version.as_deref(),
        os_info: request.os_info.as_deref(),
        content: &request.content,
    };
    let id = match diesel::insert_into(client_log_uploads::table)
        .values(&new_upload)
        .returning(client_log_uploads::id)
        .get_result::<Uuid>(&mut conn)
    {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to insert client log upload: {e}");
            return ApiError::internal("Failed to store log upload").into_response();
        }
    };

    (StatusCode::CREATED, Json(CreateClientLogResponse { id })).into_response()
}
