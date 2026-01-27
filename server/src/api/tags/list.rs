use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::schema::{recipe_version_tags, recipe_versions, recipes, user_tags};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use diesel::dsl::count;
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TagItem {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    /// Number of recipes using this tag
    pub recipe_count: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TagsListResponse {
    pub tags: Vec<TagItem>,
}

// Type alias for query result row
type TagRow = (Uuid, String, DateTime<Utc>, i64);

#[utoipa::path(
    get,
    path = "/api/tags",
    tag = "tags",
    operation_id = "list_all_tags",
    responses(
        (status = 200, description = "List of user's tags with IDs and recipe counts", body = TagsListResponse),
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

    // Use Diesel DSL with JOINs and GROUP BY instead of raw SQL subquery
    // Query: user_tags LEFT JOIN recipe_version_tags LEFT JOIN recipe_versions LEFT JOIN recipes
    // where recipes.current_version_id matches and deleted_at IS NULL
    let tags: Vec<TagRow> = match user_tags::table
        .left_join(recipe_version_tags::table)
        .left_join(
            recipe_versions::table
                .on(recipe_versions::id.eq(recipe_version_tags::recipe_version_id)),
        )
        .left_join(
            recipes::table.on(recipes::current_version_id
                .eq(recipe_versions::id.nullable())
                .and(recipes::deleted_at.is_null())),
        )
        .filter(user_tags::user_id.eq(user.id))
        .group_by((user_tags::id, user_tags::name, user_tags::created_at))
        .select((
            user_tags::id,
            user_tags::name,
            user_tags::created_at,
            count(recipes::id.nullable()),
        ))
        .order(user_tags::name.asc())
        .load(&mut conn)
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("Failed to fetch tags: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch tags".to_string(),
                }),
            )
                .into_response();
        }
    };

    let response = TagsListResponse {
        tags: tags
            .into_iter()
            .map(|(id, name, created_at, recipe_count)| TagItem {
                id,
                name,
                created_at,
                recipe_count,
            })
            .collect(),
    };

    (StatusCode::OK, Json(response)).into_response()
}
