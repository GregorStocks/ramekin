use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::schema::user_tags;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
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
) -> impl IntoResponse {
    let new_name = request.name.trim();

    if new_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Tag name cannot be empty".to_string(),
            }),
        )
            .into_response();
    }

    let mut conn = get_conn!(pool);

    // Check if tag exists, belongs to user, and is not deleted
    let existing_tag: Option<(Uuid, String)> = user_tags::table
        .filter(user_tags::id.eq(id))
        .filter(user_tags::user_id.eq(user.id))
        .filter(user_tags::deleted_at.is_null())
        .select((user_tags::id, user_tags::name))
        .first(&mut conn)
        .optional()
        .unwrap_or(None);

    let Some((_tag_id, current_name)) = existing_tag else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Tag not found".to_string(),
            }),
        )
            .into_response();
    };

    // If renaming to the same name (possibly different case), just return success
    // CITEXT comparison handles case-insensitivity
    if current_name.eq_ignore_ascii_case(new_name) {
        // Update to preserve the new casing
        let result: Result<(Uuid, String), _> = diesel::update(
            user_tags::table
                .filter(user_tags::id.eq(id))
                .filter(user_tags::user_id.eq(user.id)),
        )
        .set(user_tags::name.eq(new_name))
        .returning((user_tags::id, user_tags::name))
        .get_result(&mut conn);

        return match result {
            Ok((id, name)) => {
                (StatusCode::OK, Json(RenameTagResponse { id, name })).into_response()
            }
            Err(e) => {
                tracing::error!("Failed to rename tag: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to rename tag".to_string(),
                    }),
                )
                    .into_response()
            }
        };
    }

    // Check if another non-deleted tag with the new name already exists (case-insensitive)
    let duplicate: Option<Uuid> = user_tags::table
        .filter(user_tags::user_id.eq(user.id))
        .filter(user_tags::name.eq(new_name))
        .filter(user_tags::id.ne(id))
        .filter(user_tags::deleted_at.is_null())
        .select(user_tags::id)
        .first(&mut conn)
        .optional()
        .unwrap_or(None);

    if duplicate.is_some() {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "Tag with that name already exists".to_string(),
            }),
        )
            .into_response();
    }

    // Perform the rename
    let result: Result<(Uuid, String), _> = diesel::update(
        user_tags::table
            .filter(user_tags::id.eq(id))
            .filter(user_tags::user_id.eq(user.id)),
    )
    .set(user_tags::name.eq(new_name))
    .returning((user_tags::id, user_tags::name))
    .get_result(&mut conn);

    match result {
        Ok((id, name)) => (StatusCode::OK, Json(RenameTagResponse { id, name })).into_response(),
        Err(e) => {
            tracing::error!("Failed to rename tag: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to rename tag".to_string(),
                }),
            )
                .into_response()
        }
    }
}
