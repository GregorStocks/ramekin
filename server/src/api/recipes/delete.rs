use crate::api::{run_db_write, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::{next_sync_stamp, DbPool};
use crate::ids::Uuid;
use crate::schema::recipes;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use diesel::prelude::*;
use std::sync::Arc;

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

    // Soft delete: set deleted_at and the sync stamp that tells clients of
    // it, together (the table's CHECK pairs them). Deleting nothing is an
    // error, so the transaction rolls back and the clock doesn't move.
    run_db_write(&pool, move |conn| {
        let deleted_seq = next_sync_stamp(conn)?;
        let updated = diesel::update(
            recipes::table
                .filter(recipes::id.eq(id))
                .filter(recipes::user_id.eq(user_id))
                .filter(recipes::deleted_at.is_null()),
        )
        .set((
            recipes::deleted_at.eq(Some(Utc::now())),
            recipes::deleted_seq.eq(Some(deleted_seq)),
        ))
        .execute(conn)?;
        if updated == 0 {
            return Err(ApiError::not_found("Recipe not found"));
        }
        Ok(())
    })
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
