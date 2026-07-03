use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::ClientLogUpload;
use crate::schema::client_log_uploads;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct GetClientLogResponse {
    pub id: Uuid,
    pub platform: String,
    pub app_version: Option<String>,
    pub os_info: Option<String>,
    pub created_at: DateTime<Utc>,
    pub content: String,
}

#[utoipa::path(
    get,
    path = "/api/client-logs/{id}",
    tag = "client_logs",
    params(("id" = Uuid, Path, description = "Log upload id")),
    responses(
        (status = 200, description = "Full log upload", body = GetClientLogResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_client_log(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);
    let row: Option<ClientLogUpload> = match client_log_uploads::table
        .filter(client_log_uploads::id.eq(id))
        .filter(client_log_uploads::user_id.eq(user.id))
        .filter(client_log_uploads::deleted_at.is_null())
        .select(ClientLogUpload::as_select())
        .first(&mut conn)
        .optional()
    {
        Ok(row) => row,
        Err(e) => {
            tracing::error!("Failed to fetch client log upload: {e}");
            return ApiError::internal("Failed to fetch log upload").into_response();
        }
    };

    let Some(row) = row else {
        return ApiError::not_found("Log upload not found").into_response();
    };

    Json(GetClientLogResponse {
        id: row.id,
        platform: row.platform,
        app_version: row.app_version,
        os_info: row.os_info,
        created_at: row.created_at,
        content: row.content,
    })
    .into_response()
}
