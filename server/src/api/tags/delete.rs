use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::raw_sql;
use crate::schema::user_tags;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use diesel::prelude::*;
use std::sync::Arc;
use uuid::Uuid;

#[utoipa::path(
    delete,
    path = "/api/tags/{id}",
    tag = "tags",
    params(
        ("id" = Uuid, Path, description = "Tag ID")
    ),
    responses(
        (status = 204, description = "Tag deleted successfully"),
        (status = 404, description = "Tag not found", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn delete_tag(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;

    let updated = run_db(&pool, move |conn| {
        let now = Utc::now();

        // Soft delete - set deleted_at timestamp
        diesel::update(
            user_tags::table
                .filter(user_tags::id.eq(id))
                .filter(user_tags::user_id.eq(user_id))
                .filter(user_tags::deleted_at.is_null()),
        )
        .set((
            user_tags::deleted_at.eq(Some(now)),
            user_tags::updated_at.eq(now),
            user_tags::change_xid.eq(raw_sql::current_change_xid()),
        ))
        .execute(conn)
        .map_err(|e| {
            tracing::error!("Failed to delete tag: {}", e);
            ApiError::internal("Failed to delete tag")
        })
    })
    .await?;

    if updated == 0 {
        return Err(ApiError::not_found("Tag not found"));
    }

    Ok(StatusCode::NO_CONTENT)
}
