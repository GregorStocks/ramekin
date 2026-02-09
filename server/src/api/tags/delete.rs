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
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    // Soft delete - set deleted_at timestamp
    let updated = diesel::update(
        user_tags::table
            .filter(user_tags::id.eq(id))
            .filter(user_tags::user_id.eq(user.id))
            .filter(user_tags::deleted_at.is_null()),
    )
    .set(user_tags::deleted_at.eq(Some(Utc::now())))
    .execute(&mut conn);

    match updated {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Tag not found".to_string(),
            }),
        )
            .into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!("Failed to delete tag: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to delete tag".to_string(),
                }),
            )
                .into_response()
        }
    }
}
