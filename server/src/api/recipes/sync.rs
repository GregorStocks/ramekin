use crate::api::recipes::list::RecipeSummary;
use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::raw_sql;
use crate::schema::{recipe_versions, recipes};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Deserialize, IntoParams)]
pub struct SyncRecipesParams {
    /// Last sync timestamp - server will return changes since this time.
    pub last_sync_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncRecipesResponse {
    /// Active recipes created or updated since last_sync_at. All active recipes are returned when last_sync_at is absent.
    pub recipes: Vec<RecipeSummary>,
    /// Recipe IDs deleted since last_sync_at.
    pub deleted: Vec<Uuid>,
    /// New sync timestamp to use for the next sync.
    pub sync_timestamp: DateTime<Utc>,
}

type RecipeSyncRow = (
    Uuid,              // recipe id
    DateTime<Utc>,     // recipe created_at
    String,            // version title
    Option<String>,    // version description
    Vec<Option<Uuid>>, // version photo_ids
    Option<i32>,       // version rating
    DateTime<Utc>,     // version created_at (updated_at)
    Vec<String>,       // tags from correlated subquery
);

#[utoipa::path(
    get,
    path = "/api/recipes/sync",
    tag = "recipes",
    params(SyncRecipesParams),
    responses(
        (status = 200, description = "Recipe changes for local cache sync", body = SyncRecipesResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn sync_recipes(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Query(params): Query<SyncRecipesParams>,
) -> impl IntoResponse {
    let sync_timestamp = Utc::now();
    let mut conn = get_conn!(pool);

    let mut query = recipes::table
        .inner_join(
            recipe_versions::table.on(recipe_versions::id
                .nullable()
                .eq(recipes::current_version_id)),
        )
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .filter(recipe_versions::created_at.le(sync_timestamp))
        .into_boxed();

    if let Some(last_sync_at) = params.last_sync_at {
        query = query.filter(recipe_versions::created_at.gt(last_sync_at));
    }

    let rows: Vec<RecipeSyncRow> = match query
        .select((
            recipes::id,
            recipes::created_at,
            recipe_versions::title,
            recipe_versions::description,
            recipe_versions::photo_ids,
            recipe_versions::rating,
            recipe_versions::created_at,
            raw_sql::tags_subquery(),
        ))
        .order((recipe_versions::created_at.desc(), recipes::id.asc()))
        .load(&mut conn)
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("Failed to sync recipes: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to sync recipes".to_string(),
                }),
            )
                .into_response();
        }
    };

    let deleted = if let Some(last_sync_at) = params.last_sync_at {
        match recipes::table
            .filter(recipes::user_id.eq(user.id))
            .filter(recipes::deleted_at.is_not_null())
            .filter(recipes::deleted_at.gt(last_sync_at))
            .filter(recipes::deleted_at.le(sync_timestamp))
            .select(recipes::id)
            .load(&mut conn)
        {
            Ok(ids) => ids,
            Err(e) => {
                tracing::error!("Failed to sync deleted recipes: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to sync recipes".to_string(),
                    }),
                )
                    .into_response();
            }
        }
    } else {
        Vec::new()
    };

    let recipes = rows
        .into_iter()
        .map(
            |(id, created_at, title, description, photo_ids, rating, updated_at, tags)| {
                RecipeSummary {
                    id,
                    title,
                    description,
                    tags,
                    thumbnail_photo_id: photo_ids.first().and_then(|id| *id),
                    rating,
                    created_at,
                    updated_at,
                }
            },
        )
        .collect();

    (
        StatusCode::OK,
        Json(SyncRecipesResponse {
            recipes,
            deleted,
            sync_timestamp,
        }),
    )
        .into_response()
}
