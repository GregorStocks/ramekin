use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::raw_sql;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::Uuid as DieselUuid;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TagsResponse {
    /// List of distinct tags used across user's recipes, sorted alphabetically
    pub tags: Vec<String>,
}

#[derive(QueryableByName)]
struct TagRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    tag: String,
}

#[utoipa::path(
    get,
    path = "/api/recipes/tags",
    tag = "recipes",
    responses(
        (status = 200, description = "List of distinct tags", body = TagsResponse),
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

    // Tags are now in recipe_versions, join via current_version_id
    let tags: Vec<TagRow> = match sql_query(raw_sql::DISTINCT_TAGS_QUERY)
        .bind::<DieselUuid, _>(user.id)
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

    let response = TagsResponse {
        tags: tags.into_iter().map(|r| r.tag).collect(),
    };

    (StatusCode::OK, Json(response)).into_response()
}
