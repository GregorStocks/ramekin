use crate::api::recipes::read::{
    current_recipe_versions_for_user, recipe_relevance_select, RecipeRelevanceRow,
};
use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::Ingredient;
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
    pub recipes: Vec<SyncRecipe>,
    /// Recipe IDs deleted since last_sync_at.
    pub deleted: Vec<Uuid>,
    /// New sync timestamp to use for the next sync.
    pub sync_timestamp: DateTime<Utc>,
}

/// Read-only recipe data needed to populate the iOS cache and mirror server search.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncRecipe {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub thumbnail_photo_id: Option<Uuid>,
    pub rating: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub ingredients: Vec<Ingredient>,
    pub instructions: String,
    pub notes: Option<String>,
}

impl SyncRecipe {
    fn try_from_row(row: RecipeRelevanceRow) -> Result<Self, serde_json::Error> {
        let ingredients = serde_json::from_value(row.ingredients)?;
        Ok(Self {
            id: row.id,
            title: row.title,
            description: row.description,
            tags: row.tags,
            thumbnail_photo_id: row.photo_ids.first().and_then(|id| *id),
            rating: row.rating,
            created_at: row.created_at,
            updated_at: row.updated_at,
            ingredients,
            instructions: row.instructions,
            notes: row.notes,
        })
    }
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

    let rows: Vec<RecipeRelevanceRow> = match query
        .select(recipe_relevance_select!())
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

    let recipes = match rows
        .into_iter()
        .map(SyncRecipe::try_from_row)
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(recipes) => recipes,
        Err(e) => {
            tracing::error!(error = %e, "stored ingredients JSON failed to deserialize during recipe sync");
            return ApiError::internal("Recipe ingredients are corrupt").into_response();
        }
    };

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
