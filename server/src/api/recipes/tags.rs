use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::schema::user_tags;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TagsResponse {
    /// List of all user's tags, sorted alphabetically
    pub tags: Vec<String>,
}

#[utoipa::path(
    get,
    path = "/api/recipes/tags",
    tag = "recipes",
    responses(
        (status = 200, description = "List of user's tags", body = TagsResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_tags(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    // Query tags from user_tags table directly
    let tags: Vec<String> = match user_tags::table
        .filter(user_tags::user_id.eq(user.id))
        .select(user_tags::name)
        .order(user_tags::name.asc())
        .load(&mut conn)
    {
        Ok(rows) => rows,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch tags".to_string(),
                }),
            )
                .into_response()
        }
    };

    let response = TagsResponse { tags };

    (StatusCode::OK, Json(response)).into_response()
}
