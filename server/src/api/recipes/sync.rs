use crate::api::recipes::list::RecipeSummary;
use crate::api::recipes::read::{
    current_recipe_versions_for_user, recipe_summary_select, RecipeSummaryRow,
};
use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::schema::{recipe_version_tags, recipe_versions, recipes, user_tags};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use diesel::dsl::exists;
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

    let mut query = current_recipe_versions_for_user!(user.id)
        .filter(recipe_versions::created_at.le(sync_timestamp))
        .into_boxed();

    if let Some(last_sync_at) = params.last_sync_at {
        let tag_changed = exists(
            recipe_version_tags::table
                .inner_join(user_tags::table)
                .filter(recipe_version_tags::recipe_version_id.eq(recipe_versions::id))
                .filter(user_tags::updated_at.gt(last_sync_at))
                .filter(user_tags::updated_at.le(sync_timestamp)),
        );
        query = query.filter(recipe_versions::created_at.gt(last_sync_at).or(tag_changed));
    }

    let rows: Vec<RecipeSummaryRow> = match query
        .select(recipe_summary_select!())
        .order((recipe_versions::created_at.desc(), recipes::id.asc()))
        .load(&mut conn)
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("Failed to sync recipes: {}", e);
            return ApiError::internal("Failed to sync recipes").into_response();
        }
    };

    let mut deleted_query = recipes::table
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_not_null())
        .filter(recipes::deleted_at.le(sync_timestamp))
        .into_boxed();

    if let Some(last_sync_at) = params.last_sync_at {
        deleted_query = deleted_query.filter(recipes::deleted_at.gt(last_sync_at));
    }

    let deleted = match deleted_query.select(recipes::id).load(&mut conn) {
        Ok(ids) => ids,
        Err(e) => {
            tracing::error!("Failed to sync deleted recipes: {}", e);
            return ApiError::internal("Failed to sync recipes").into_response();
        }
    };

    let recipes = rows.into_iter().map(RecipeSummary::from_row).collect();

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
