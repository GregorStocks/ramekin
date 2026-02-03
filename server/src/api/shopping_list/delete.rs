use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::schema::shopping_list_items;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
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
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    // Hard delete - no soft delete for shopping list items
    let deleted = match diesel::delete(
        shopping_list_items::table
            .filter(shopping_list_items::id.eq(id))
            .filter(shopping_list_items::user_id.eq(user.id)),
    )
    .execute(&mut conn)
    {
        Ok(count) => count,
        Err(e) => {
            tracing::error!("Failed to delete shopping list item: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to delete item".to_string(),
                }),
            )
                .into_response();
        }
    };

    if deleted == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Item not found".to_string(),
            }),
        )
            .into_response();
    }

    StatusCode::NO_CONTENT.into_response()
}
