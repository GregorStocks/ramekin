use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::schema::shopping_list_items;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ShoppingListItemResponse {
    pub id: Uuid,
    pub item: String,
    pub amount: Option<String>,
    pub note: Option<String>,
    pub source_recipe_id: Option<Uuid>,
    pub source_recipe_title: Option<String>,
    pub is_checked: bool,
    pub sort_order: i32,
    pub version: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ShoppingListResponse {
    pub items: Vec<ShoppingListItemResponse>,
}

// Type alias for query result row
type ShoppingListRow = (
    Uuid,
    String,
    Option<String>,
    Option<String>,
    Option<Uuid>,
    Option<String>,
    bool,
    i32,
    i32,
    DateTime<Utc>,
);

#[utoipa::path(
    get,
    path = "/api/shopping-list",
    tag = "shopping_list",
    responses(
        (status = 200, description = "List of shopping list items", body = ShoppingListResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_items(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    let rows: Vec<ShoppingListRow> = match shopping_list_items::table
        .filter(shopping_list_items::user_id.eq(user.id))
        .filter(shopping_list_items::deleted_at.is_null())
        .select((
            shopping_list_items::id,
            shopping_list_items::item,
            shopping_list_items::amount,
            shopping_list_items::note,
            shopping_list_items::source_recipe_id,
            shopping_list_items::source_recipe_title,
            shopping_list_items::is_checked,
            shopping_list_items::sort_order,
            shopping_list_items::version,
            shopping_list_items::updated_at,
        ))
        .order((
            shopping_list_items::is_checked.asc(),
            shopping_list_items::sort_order.asc(),
        ))
        .load(&mut conn)
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("Failed to fetch shopping list: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch shopping list".to_string(),
                }),
            )
                .into_response();
        }
    };

    let items = rows
        .into_iter()
        .map(
            |(
                id,
                item,
                amount,
                note,
                source_recipe_id,
                source_recipe_title,
                is_checked,
                sort_order,
                version,
                updated_at,
            )| {
                ShoppingListItemResponse {
                    id,
                    item,
                    amount,
                    note,
                    source_recipe_id,
                    source_recipe_title,
                    is_checked,
                    sort_order,
                    version,
                    updated_at,
                }
            },
        )
        .collect();

    (StatusCode::OK, Json(ShoppingListResponse { items })).into_response()
}
