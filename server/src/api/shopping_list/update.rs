use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
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
use serde_with::rust::double_option;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

// Type alias for query result row
type ItemRow = (
    String,
    Option<String>,
    Option<String>,
    bool,
    i32,
    Option<String>,
    i32,
);

#[derive(Debug, Clone, Deserialize, ToSchema, Default)]
pub struct UpdateShoppingListItemRequest {
    pub item: Option<String>,
    pub amount: Option<String>,
    pub note: Option<String>,
    pub is_checked: Option<bool>,
    pub sort_order: Option<i32>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<String>)]
    pub category_override: Option<Option<String>>,
    pub clear_category_override: Option<bool>,
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
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;
    run_db(&pool, move |conn| {
        // Fetch the existing item
        let existing: Option<ItemRow> = shopping_list_items::table
            .filter(shopping_list_items::id.eq(id))
            .filter(shopping_list_items::user_id.eq(user_id))
            .filter(shopping_list_items::deleted_at.is_null())
            .select((
                shopping_list_items::item,
                shopping_list_items::amount,
                shopping_list_items::note,
                shopping_list_items::is_checked,
                shopping_list_items::sort_order,
                shopping_list_items::category_override,
                shopping_list_items::version,
            ))
            .first(conn)
            .optional()
            .map_err(|e| {
                tracing::error!("Failed to fetch shopping list item: {}", e);
                ApiError::internal("Failed to fetch item")
            })?;

        let Some((
            current_item,
            current_amount,
            current_note,
            current_checked,
            current_order,
            current_category_override,
            current_version,
        )) = existing
        else {
            return Err(ApiError::not_found("Item not found"));
        };

        // Calculate new values
        let new_item = request.item.unwrap_or(current_item);
        let new_amount = request.amount.or(current_amount);
        let new_note = request.note.or(current_note);
        let new_checked = request.is_checked.unwrap_or(current_checked);
        let new_order = request.sort_order.unwrap_or(current_order);
        if request.clear_category_override == Some(true)
            && matches!(request.category_override, Some(Some(_)))
        {
            return Err(ApiError::invalid_request(
                "Conflicting category override fields",
            ));
        }
        let new_category_override = if request.clear_category_override == Some(true) {
            None
        } else {
            request
                .category_override
                .unwrap_or(current_category_override)
        };

        if new_category_override
            .as_deref()
            .is_some_and(|category| !super::list::is_valid_category(category))
        {
            return Err(ApiError::invalid_request("Invalid category override"));
        }

        // Update the item
        let result = diesel::update(
            shopping_list_items::table
                .filter(shopping_list_items::id.eq(id))
                .filter(shopping_list_items::user_id.eq(user_id))
                .filter(shopping_list_items::deleted_at.is_null()),
        )
        .set((
            shopping_list_items::item.eq(&new_item),
            shopping_list_items::amount.eq(&new_amount),
            shopping_list_items::note.eq(&new_note),
            shopping_list_items::is_checked.eq(new_checked),
            shopping_list_items::sort_order.eq(new_order),
            shopping_list_items::category_override.eq(&new_category_override),
            shopping_list_items::version.eq(current_version + 1),
            shopping_list_items::updated_at.eq(Utc::now()),
        ))
        .execute(conn);

        match result {
            Ok(0) => Err(ApiError::not_found("Item not found")),
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!("Failed to update shopping list item: {}", e);
                Err(ApiError::internal("Failed to update item"))
            }
        }
    })
    .await?;

    Ok(StatusCode::OK)
}
