use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::schema::shopping_list_items;
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
    path = "/api/shopping-list/{id}",
    tag = "shopping_list",
    params(
        ("id" = Uuid, Path, description = "Shopping list item ID")
    ),
    responses(
        (status = 204, description = "Item deleted"),
        (status = 404, description = "Item not found", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_item(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;
    run_db(&pool, move |conn| {
        let now = Utc::now();
        let deleted = diesel::update(
            shopping_list_items::table
                .filter(shopping_list_items::id.eq(id))
                .filter(shopping_list_items::user_id.eq(user_id))
                .filter(shopping_list_items::deleted_at.is_null()),
        )
        .set((
            shopping_list_items::deleted_at.eq(now),
            shopping_list_items::updated_at.eq(now),
            shopping_list_items::version.eq(shopping_list_items::version + 1),
        ))
        .execute(conn)
        .map_err(|e| {
            tracing::error!("Failed to delete shopping list item: {}", e);
            ApiError::internal("Failed to delete item")
        })?;

        if deleted == 0 {
            let exists = shopping_list_items::table
                .filter(shopping_list_items::id.eq(id))
                .filter(shopping_list_items::user_id.eq(user_id))
                .select(shopping_list_items::id)
                .first::<Uuid>(conn)
                .optional()
                .map_err(|e| {
                    tracing::error!("Failed to check shopping list item: {}", e);
                    ApiError::internal("Failed to delete item")
                })?;

            if exists.is_none() {
                return Err(ApiError::not_found("Item not found"));
            }
        }

        Ok(())
    })
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
