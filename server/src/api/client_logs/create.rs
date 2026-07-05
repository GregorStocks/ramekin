use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use axum::{http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::env;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
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

#[derive(Debug, Clone, Serialize)]
struct StoredClientLogUpload {
    id: Uuid,
    user_id: Uuid,
    platform: String,
    app_version: Option<String>,
    os_info: Option<String>,
    created_at: DateTime<Utc>,
    content: String,
}

fn client_log_dir() -> PathBuf {
    env::var_os("RAMEKIN_CLIENT_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../logs/client-logs"))
}

async fn ensure_private_client_log_dir(dir: &PathBuf) -> std::io::Result<()> {
    tokio::fs::create_dir_all(dir).await?;

    #[cfg(unix)]
    tokio::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).await?;

    Ok(())
}

async fn write_private_client_log_file(path: &PathBuf, body: &[u8]) -> std::io::Result<()> {
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);

    #[cfg(unix)]
    options.mode(0o600);

    let mut file = options.open(path).await?;
    file.write_all(body).await?;
    file.flush().await
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

    let upload = StoredClientLogUpload {
        id: Uuid::new_v4(),
        user_id: user.id,
        platform: request.platform,
        app_version: request.app_version,
        os_info: request.os_info,
        created_at: Utc::now(),
        content: request.content,
    };

    let dir = client_log_dir();
    if let Err(e) = ensure_private_client_log_dir(&dir).await {
        tracing::error!("Failed to create client log directory {:?}: {e}", dir);
        return ApiError::internal("Failed to store log upload").into_response();
    }

    let path = dir.join(format!("{}.json", upload.id));
    let mut body = match serde_json::to_vec_pretty(&upload) {
        Ok(body) => body,
        Err(e) => {
            tracing::error!("Failed to serialize client log upload: {e}");
            return ApiError::internal("Failed to store log upload").into_response();
        }
    };
    body.push(b'\n');

    if let Err(e) = write_private_client_log_file(&path, &body).await {
        tracing::error!("Failed to write client log upload {:?}: {e}", path);
        return ApiError::internal("Failed to store log upload").into_response();
    }

    tracing::info!(upload_id = %upload.id, user_id = %upload.user_id, path = ?path, "stored client log upload");

    (
        StatusCode::CREATED,
        Json(CreateClientLogResponse { id: upload.id }),
    )
        .into_response()
}
