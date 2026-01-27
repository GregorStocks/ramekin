use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::UserTag;
use crate::schema::user_tags;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TagItem {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TagsListResponse {
    pub tags: Vec<TagItem>,
}

#[utoipa::path(
    get,
    path = "/api/tags",
    tag = "tags",
    operation_id = "list_all_tags",
    responses(
        (status = 200, description = "List of user's tags with IDs", body = TagsListResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_all_tags(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    let tags: Vec<UserTag> = match user_tags::table
        .filter(user_tags::user_id.eq(user.id))
        .select(UserTag::as_select())
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

    let response = TagsListResponse {
        tags: tags
            .into_iter()
            .map(|t| TagItem {
                id: t.id,
                name: t.name,
                created_at: t.created_at,
            })
            .collect(),
    };

    (StatusCode::OK, Json(response)).into_response()
}
