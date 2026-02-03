use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::NewShoppingListItem;
use crate::schema::shopping_list_items;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateShoppingListItemRequest {
    pub item: String,
    pub amount: Option<String>,
    pub note: Option<String>,
    pub source_recipe_id: Option<Uuid>,
    pub source_recipe_title: Option<String>,
    /// Client-generated ID for offline sync
    pub client_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateShoppingListRequest {
    pub items: Vec<CreateShoppingListItemRequest>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CreateShoppingListResponse {
    pub ids: Vec<Uuid>,
}

#[utoipa::path(
    post,
    path = "/api/shopping-list",
    tag = "shopping_list",
    request_body = CreateShoppingListRequest,
    responses(
        (status = 201, description = "Items created", body = CreateShoppingListResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_items(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<CreateShoppingListRequest>,
) -> impl IntoResponse {
    if request.items.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "At least one item is required".to_string(),
            }),
        )
            .into_response();
    }

    let mut conn = get_conn!(pool);

    // Get current max sort_order for this user
    let max_sort_order: i32 = shopping_list_items::table
        .filter(shopping_list_items::user_id.eq(user.id))
        .select(diesel::dsl::max(shopping_list_items::sort_order))
        .first::<Option<i32>>(&mut conn)
        .unwrap_or(None)
        .unwrap_or(0);

    let mut ids = Vec::with_capacity(request.items.len());

    for (i, item_req) in request.items.iter().enumerate() {
        let amount_ref = item_req.amount.as_deref();
        let note_ref = item_req.note.as_deref();
        let source_title_ref = item_req.source_recipe_title.as_deref();

        let new_item = NewShoppingListItem {
            user_id: user.id,
            item: &item_req.item,
            amount: amount_ref,
            note: note_ref,
            source_recipe_id: item_req.source_recipe_id,
            source_recipe_title: source_title_ref,
            is_checked: false,
            sort_order: max_sort_order + 1 + i as i32,
            client_id: item_req.client_id,
        };

        match diesel::insert_into(shopping_list_items::table)
            .values(&new_item)
            .returning(shopping_list_items::id)
            .get_result::<Uuid>(&mut conn)
        {
            Ok(id) => ids.push(id),
            Err(e) => {
                tracing::error!("Failed to create shopping list item: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to create shopping list item".to_string(),
                    }),
                )
                    .into_response();
            }
        }
    }

    (
        StatusCode::CREATED,
        Json(CreateShoppingListResponse { ids }),
    )
        .into_response()
}
