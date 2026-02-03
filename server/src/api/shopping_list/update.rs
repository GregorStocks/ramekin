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
use chrono::Utc;
use diesel::prelude::*;
use serde::Deserialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

// Type alias for query result row
type ItemRow = (String, Option<String>, Option<String>, bool, i32, i32);

#[derive(Debug, Clone, Deserialize, ToSchema, Default)]
pub struct UpdateShoppingListItemRequest {
    pub item: Option<String>,
    pub amount: Option<String>,
    pub note: Option<String>,
    pub is_checked: Option<bool>,
    pub sort_order: Option<i32>,
}

#[utoipa::path(
    put,
    path = "/api/shopping-list/{id}",
    tag = "shopping_list",
    params(
        ("id" = Uuid, Path, description = "Shopping list item ID")
    ),
    request_body = UpdateShoppingListItemRequest,
    responses(
        (status = 200, description = "Item updated"),
        (status = 404, description = "Item not found", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_item(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateShoppingListItemRequest>,
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    // Fetch the existing item
    let existing: Option<ItemRow> = match shopping_list_items::table
        .filter(shopping_list_items::id.eq(id))
        .filter(shopping_list_items::user_id.eq(user.id))
        .select((
            shopping_list_items::item,
            shopping_list_items::amount,
            shopping_list_items::note,
            shopping_list_items::is_checked,
            shopping_list_items::sort_order,
            shopping_list_items::version,
        ))
        .first(&mut conn)
        .optional()
    {
        Ok(record) => record,
        Err(e) => {
            tracing::error!("Failed to fetch shopping list item: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch item".to_string(),
                }),
            )
                .into_response();
        }
    };

    let Some((
        current_item,
        current_amount,
        current_note,
        current_checked,
        current_order,
        current_version,
    )) = existing
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Item not found".to_string(),
            }),
        )
            .into_response();
    };

    // Calculate new values
    let new_item = request.item.unwrap_or(current_item);
    let new_amount = request.amount.or(current_amount);
    let new_note = request.note.or(current_note);
    let new_checked = request.is_checked.unwrap_or(current_checked);
    let new_order = request.sort_order.unwrap_or(current_order);

    // Update the item
    let result = diesel::update(
        shopping_list_items::table
            .filter(shopping_list_items::id.eq(id))
            .filter(shopping_list_items::user_id.eq(user.id)),
    )
    .set((
        shopping_list_items::item.eq(&new_item),
        shopping_list_items::amount.eq(&new_amount),
        shopping_list_items::note.eq(&new_note),
        shopping_list_items::is_checked.eq(new_checked),
        shopping_list_items::sort_order.eq(new_order),
        shopping_list_items::version.eq(current_version + 1),
        shopping_list_items::updated_at.eq(Utc::now()),
    ))
    .execute(&mut conn);

    match result {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Item not found".to_string(),
            }),
        )
            .into_response(),
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => {
            tracing::error!("Failed to update shopping list item: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to update item".to_string(),
                }),
            )
                .into_response()
        }
    }
}
