use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::raw_sql;
use crate::schema::user_tags;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RenameTagRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RenameTagResponse {
    pub id: Uuid,
    pub name: String,
}

#[utoipa::path(
    patch,
    path = "/api/tags/{id}",
    tag = "tags",
    params(
        ("id" = Uuid, Path, description = "Tag ID")
    ),
    request_body = RenameTagRequest,
    responses(
        (status = 200, description = "Tag renamed successfully", body = RenameTagResponse),
        (status = 400, description = "Invalid request (empty name)", body = ErrorResponse),
        (status = 404, description = "Tag not found", body = ErrorResponse),
        (status = 409, description = "Tag with that name already exists", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn rename_tag(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
    Json(request): Json<RenameTagRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let new_name = request.name.trim().to_string();

    if let Err(err) = ramekin_core::validate_tag_name(&new_name) {
        return Err(ApiError::invalid_request(err.message().to_string()));
    }

    let user_id = user.id;

    let (id, name) = run_db(&pool, move |conn| {
        // Check if tag exists, belongs to user, and is not deleted
        let existing_tag: Option<(Uuid, String)> = user_tags::table
            .filter(user_tags::id.eq(id))
            .filter(user_tags::user_id.eq(user_id))
            .filter(user_tags::deleted_at.is_null())
            .select((user_tags::id, user_tags::name))
            .first(conn)
            .optional()
            .map_err(|e| {
                tracing::error!("Failed to look up tag: {}", e);
                ApiError::internal("Failed to look up tag")
            })?;

        let Some((_tag_id, current_name)) = existing_tag else {
            return Err(ApiError::not_found("Tag not found"));
        };

        // If renaming to the same name (possibly different case), just return success
        // CITEXT comparison handles case-insensitivity
        if current_name.eq_ignore_ascii_case(&new_name) {
            // Update to preserve the new casing
            let now = Utc::now();
            return diesel::update(
                user_tags::table
                    .filter(user_tags::id.eq(id))
                    .filter(user_tags::user_id.eq(user_id)),
            )
            .set((
                user_tags::name.eq(new_name.as_str()),
                user_tags::updated_at.eq(now),
                user_tags::change_xid.eq(raw_sql::current_change_xid()),
            ))
            .returning((user_tags::id, user_tags::name))
            .get_result(conn)
            .map_err(|e| {
                tracing::error!("Failed to rename tag: {}", e);
                ApiError::internal("Failed to rename tag")
            });
        }

        // Check if another non-deleted tag with the new name already exists (case-insensitive)
        let duplicate: Option<Uuid> = user_tags::table
            .filter(user_tags::user_id.eq(user_id))
            .filter(user_tags::name.eq(new_name.as_str()))
            .filter(user_tags::id.ne(id))
            .filter(user_tags::deleted_at.is_null())
            .select(user_tags::id)
            .first(conn)
            .optional()
            .map_err(|e| {
                tracing::error!("Failed to check for duplicate tag: {}", e);
                ApiError::internal("Failed to check for duplicate tag")
            })?;

        if duplicate.is_some() {
            return Err(ApiError::conflict("Tag with that name already exists"));
        }

        // Perform the rename
        let now = Utc::now();
        diesel::update(
            user_tags::table
                .filter(user_tags::id.eq(id))
                .filter(user_tags::user_id.eq(user_id)),
        )
        .set((
            user_tags::name.eq(new_name.as_str()),
            user_tags::updated_at.eq(now),
            user_tags::change_xid.eq(raw_sql::current_change_xid()),
        ))
        .returning((user_tags::id, user_tags::name))
        .get_result(conn)
        .map_err(|e| {
            tracing::error!("Failed to rename tag: {}", e);
            ApiError::internal("Failed to rename tag")
        })
    })
    .await?;

    Ok((StatusCode::OK, Json(RenameTagResponse { id, name })))
}
