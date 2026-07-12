use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::schema::shopping_list_items;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use ramekin_core::ingredient_categorizer;
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
    /// User-selected category override; when set, it wins over computed category.
    pub category_override: Option<String>,
    /// Category computed from the item name before applying any override.
    pub computed_category: String,
    /// Aisle category for grouping (override when set, otherwise computed).
    pub category: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ShoppingListResponse {
    pub items: Vec<ShoppingListItemResponse>,
    /// Canonical category display order for grouping items; every item's
    /// `category` is guaranteed to appear in this list.
    pub category_order: Vec<String>,
}

/// The canonical category display order, as served to clients.
pub fn category_order() -> Vec<String> {
    ingredient_categorizer::CATEGORIES
        .into_iter()
        .map(str::to_string)
        .collect()
}

pub fn is_valid_category(category: &str) -> bool {
    ingredient_categorizer::CATEGORIES.contains(&category)
}

pub fn computed_category(item: &str) -> String {
    ingredient_categorizer::categorize(item).to_string()
}

pub fn item_category(computed_category: &str, category_override: Option<&str>) -> String {
    category_override.unwrap_or(computed_category).to_string()
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
    Option<String>,
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
) -> Result<impl IntoResponse, ApiError> {
    let rows: Vec<ShoppingListRow> = run_db(&pool, move |conn| {
        shopping_list_items::table
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
                shopping_list_items::category_override,
                shopping_list_items::version,
                shopping_list_items::updated_at,
            ))
            .order((
                shopping_list_items::is_checked.asc(),
                shopping_list_items::sort_order.asc(),
            ))
            .load(conn)
            .map_err(|e| {
                tracing::error!("Failed to fetch shopping list: {}", e);
                ApiError::internal("Failed to fetch shopping list")
            })
    })
    .await?;

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
                category_override,
                version,
                updated_at,
            )| {
                let computed_category = computed_category(&item);
                let category = item_category(&computed_category, category_override.as_deref());
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
                    category_override,
                    computed_category,
                    category,
                }
            },
        )
        .collect();

    Ok((
        StatusCode::OK,
        Json(ShoppingListResponse {
            items,
            category_order: category_order(),
        }),
    ))
}
