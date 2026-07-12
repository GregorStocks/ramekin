use crate::api::recipes::read::{
    current_recipe_versions_for_user, recipe_relevance_select, RecipeRelevanceRow,
};
use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::Ingredient;
use crate::raw_sql;
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

/// Ceiling on the sync page size: the whole point of paging is that no single
/// response is ever huge.
const MAX_SYNC_PAGE_SIZE: i64 = 500;

#[derive(Debug, Deserialize, IntoParams)]
pub struct SyncRecipesParams {
    /// Cursor returned by a previous sync. Absent means a full sync. Every
    /// page of one sweep passes the same cursor.
    pub cursor: Option<i64>,
    /// Maximum number of recipes returned per page, between 1 and 500.
    pub limit: i64,
    /// ID of the last recipe on the previous page. Absent on a sweep's first
    /// page.
    pub after_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncRecipesResponse {
    /// The next `limit` active recipes (by ascending recipe ID, starting past
    /// `after_id`) changed at or after `cursor`. All active recipes match when
    /// `cursor` is absent.
    pub recipes: Vec<SyncRecipe>,
    /// Recipe IDs deleted at or after `cursor`. Only sent on a sweep's first
    /// page; later pages return an empty list.
    pub deleted: Vec<Uuid>,
    /// This page's snapshot watermark. Once a sweep completes, persist the
    /// *first* page's cursor and pass it to the next sync: changes committed
    /// mid-sweep can land in id ranges the sweep already passed, and only the
    /// first watermark is low enough to redeliver all of them. Changes may be
    /// redelivered across syncs, but none can be skipped.
    pub cursor: i64,
    /// True when the sweep has more pages. Request the next one with the same
    /// `cursor` and `after_id` set to this page's last recipe ID.
    pub has_more: bool,
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
        (status = 400, description = "Invalid limit", body = ErrorResponse),
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
    if !(1..=MAX_SYNC_PAGE_SIZE).contains(&params.limit) {
        return ApiError::invalid_request(format!(
            "limit must be between 1 and {MAX_SYNC_PAGE_SIZE}"
        ))
        .into_response();
    }

    let mut conn = get_conn!(pool);

    // One read-only repeatable-read transaction, so the cursor and both reads
    // come from a single snapshot. The cursor is the snapshot's xmin: the
    // lowest transaction id still in flight. Anything a writer commits after
    // this snapshot carries a change_xid at or above it, so the next sync's
    // inclusive `>= cursor` filter picks it up instead of skipping it. The
    // price is that changes can be redelivered; applying them is idempotent.
    //
    // A sweep pages through the recipe-id space under a fixed `cursor` filter.
    // Every row matching the filter that committed before the sweep began is
    // visible to every page's snapshot, so partitioning by id delivers each of
    // them exactly once. A change committed mid-sweep can land in an id range
    // the sweep already passed, but its change_xid is at or above the first
    // page's watermark — which is why clients persist the first page's cursor,
    // not the last one's.
    let loaded = conn
        .build_transaction()
        .read_only()
        .repeatable_read()
        .run(|conn| {
            // Runs first so it establishes the snapshot the reads below use.
            let cursor: i64 = diesel::select(raw_sql::change_xid_watermark()).get_result(conn)?;

            let mut query = current_recipe_versions_for_user!(user.id).into_boxed();

            if let Some(since) = params.cursor {
                // A rename or delete rewrites the tags of every recipe carrying
                // the tag, without touching the recipe's own version row.
                let tag_changed = exists(
                    recipe_version_tags::table
                        .inner_join(user_tags::table)
                        .filter(recipe_version_tags::recipe_version_id.eq(recipe_versions::id))
                        .filter(user_tags::change_xid.ge(since)),
                );
                query = query.filter(recipe_versions::change_xid.ge(since).or(tag_changed));
            }

            if let Some(after_id) = params.after_id {
                query = query.filter(recipes::id.gt(after_id));
            }

            let mut rows: Vec<RecipeRelevanceRow> = query
                .select(recipe_relevance_select!())
                .order(recipes::id.asc())
                .limit(params.limit + 1)
                .load(conn)?;

            let has_more = rows.len() as i64 > params.limit;
            rows.truncate(params.limit as usize);

            // Deletions are cheap (bare IDs) and not paged, so one page — the
            // first — carries them all and the rest skip the query.
            let deleted: Vec<Uuid> = if params.after_id.is_none() {
                let mut deleted_query = recipes::table
                    .filter(recipes::user_id.eq(user.id))
                    .filter(recipes::deleted_at.is_not_null())
                    .into_boxed();

                if let Some(since) = params.cursor {
                    deleted_query = deleted_query.filter(recipes::deleted_xid.ge(since));
                }

                deleted_query.select(recipes::id).load(conn)?
            } else {
                Vec::new()
            };

            Ok::<_, diesel::result::Error>((cursor, rows, deleted, has_more))
        });

    let (cursor, rows, deleted, has_more) = match loaded {
        Ok(loaded) => loaded,
        Err(e) => {
            tracing::error!("Failed to sync recipes: {}", e);
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
            cursor,
            has_more,
        }),
    )
        .into_response()
}
