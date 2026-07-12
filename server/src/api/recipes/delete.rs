use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
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
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;

    // Soft delete - set deleted_at timestamp
    let updated = run_db(&pool, move |conn| {
        diesel::update(
            recipes::table
                .filter(recipes::id.eq(id))
                .filter(recipes::user_id.eq(user_id))
                .filter(recipes::deleted_at.is_null()),
        )
        .set((
            recipes::deleted_at.eq(Some(Utc::now())),
            recipes::deleted_xid.eq(raw_sql::current_change_xid().nullable()),
        ))
        .execute(conn)
        .map_err(|e| {
            tracing::error!("Failed to delete recipe: {}", e);
            ApiError::internal("Failed to delete recipe")
        })
    })
    .await?;

    if updated == 0 {
        return Err(ApiError::not_found("Recipe not found"));
    }

    Ok(StatusCode::NO_CONTENT)
}
