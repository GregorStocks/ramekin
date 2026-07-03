use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::ClientLogUpload;
use crate::schema::client_log_uploads;
use axum::{extract::State, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ClientLogSummary {
    pub id: Uuid,
    pub platform: String,
    pub app_version: Option<String>,
    pub os_info: Option<String>,
    pub created_at: DateTime<Utc>,
    pub content_length: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ListClientLogsResponse {
    pub uploads: Vec<ClientLogSummary>,
}

#[utoipa::path(
    get,
    path = "/api/client-logs",
    tag = "client_logs",
    responses(
        (status = 200, description = "Caller's log uploads, newest first", body = ListClientLogsResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_client_logs(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);
    let rows: Vec<ClientLogUpload> = match client_log_uploads::table
        .filter(client_log_uploads::user_id.eq(user.id))
        .filter(client_log_uploads::deleted_at.is_null())
        .order(client_log_uploads::created_at.desc())
        .select(ClientLogUpload::as_select())
        .load(&mut conn)
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("Failed to list client log uploads: {e}");
            return ApiError::internal("Failed to list log uploads").into_response();
        }
    };

    let uploads = rows
        .into_iter()
        .map(|row| ClientLogSummary {
            id: row.id,
            platform: row.platform,
            app_version: row.app_version,
            os_info: row.os_info,
            created_at: row.created_at,
            content_length: row.content.len() as i64,
        })
        .collect();

    Json(ListClientLogsResponse { uploads }).into_response()
}
