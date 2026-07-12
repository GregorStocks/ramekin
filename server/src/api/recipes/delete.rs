use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::raw_sql;
use crate::schema::recipes;
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
    path = "/api/recipes/{id}",
    tag = "recipes",
    params(
        ("id" = Uuid, Path, description = "Recipe ID")
    ),
    responses(
        (status = 204, description = "Recipe deleted successfully"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Recipe not found", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn delete_recipe(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    // Soft delete - set deleted_at timestamp
    let updated = match diesel::update(
        recipes::table
            .filter(recipes::id.eq(id))
            .filter(recipes::user_id.eq(user.id))
            .filter(recipes::deleted_at.is_null()),
    )
    .set((
        recipes::deleted_at.eq(Some(Utc::now())),
        recipes::deleted_xid.eq(raw_sql::current_change_xid().nullable()),
    ))
    .execute(&mut conn)
    {
        Ok(count) => count,
        Err(e) => {
            tracing::error!("Failed to delete recipe: {}", e);
            return ApiError::internal("Failed to delete recipe").into_response();
        }
    };

    if updated == 0 {
        return ApiError::not_found("Recipe not found").into_response();
    }

    StatusCode::NO_CONTENT.into_response()
}
