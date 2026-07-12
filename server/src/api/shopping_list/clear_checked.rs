use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
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
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;
    let deleted = run_db(&pool, move |conn| {
        let now = Utc::now();
        diesel::update(
            shopping_list_items::table
                .filter(shopping_list_items::user_id.eq(user_id))
                .filter(shopping_list_items::is_checked.eq(true))
                .filter(shopping_list_items::deleted_at.is_null()),
        )
        .set((
            shopping_list_items::deleted_at.eq(now),
            shopping_list_items::updated_at.eq(now),
            shopping_list_items::version.eq(shopping_list_items::version + 1),
        ))
        .execute(conn)
        .map_err(|e| {
            tracing::error!("Failed to clear checked items: {}", e);
            ApiError::internal("Failed to clear checked items")
        })
    })
    .await?;

    Ok((
        StatusCode::OK,
        Json(ClearCheckedResponse {
            deleted_count: deleted,
        }),
    ))
}
