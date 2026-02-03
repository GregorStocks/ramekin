use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::schema::shopping_list_items;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::Utc;
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ClearCheckedResponse {
    pub deleted_count: usize,
}

#[utoipa::path(
    delete,
    path = "/api/shopping-list/clear-checked",
    tag = "shopping_list",
    responses(
        (status = 200, description = "Checked items cleared", body = ClearCheckedResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn clear_checked(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    let now = Utc::now();
    let deleted = match diesel::update(
        shopping_list_items::table
            .filter(shopping_list_items::user_id.eq(user.id))
            .filter(shopping_list_items::is_checked.eq(true))
            .filter(shopping_list_items::deleted_at.is_null()),
    )
    .set((
        shopping_list_items::deleted_at.eq(now),
        shopping_list_items::updated_at.eq(now),
        shopping_list_items::version.eq(shopping_list_items::version + 1),
    ))
    .execute(&mut conn)
    {
        Ok(count) => count,
        Err(e) => {
            tracing::error!("Failed to clear checked items: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to clear checked items".to_string(),
                }),
            )
                .into_response();
        }
    };

    (
        StatusCode::OK,
        Json(ClearCheckedResponse {
            deleted_count: deleted,
        }),
    )
        .into_response()
}
