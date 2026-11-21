use crate::api::recipes::read::{
    current_recipe_versions_for_user, recipe_sync_select, RecipeSyncRow,
};
use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::{self, DbPool};
use crate::ids::Uuid;
use crate::models::Ingredient;
use crate::raw_sql;
use crate::schema::{photos, recipes};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

/// Ceiling on the sync page size: the whole point of paging is that no single
/// response is ever huge.
const MAX_SYNC_PAGE_SIZE: i64 = 500;

#[derive(Debug, Deserialize, IntoParams)]
pub struct SyncRecipesParams {
    /// `cursor` from the previous response. Absent means a full sync.
    pub cursor: Option<i64>,
    /// `after_id` from the previous response, when it had one.
    pub after_id: Option<Uuid>,
    /// Maximum number of recipes returned per page, between 1 and 500.
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncRecipesResponse {
    /// The next `limit` active recipes changed since the request's position,
    /// oldest change first. A recipe changed again later comes again, later
    /// in the stream, so paging can't skip a change that lands mid-sync.
    pub changes: Vec<SyncRecipe>,
    /// Recipe IDs deleted since the request's position. A full resync sends
    /// none.
    pub deleted: Vec<Uuid>,
    /// Where the next request starts: pass it back as `cursor`, together
    /// with `after_id`. Persist the pair once this page is applied; an
    /// interrupted sync resumes from it. Changes may be redelivered, but
    /// none can be skipped.
    pub cursor: i64,
    /// Set while `has_more`: the last recipe of this page, which shares its
    /// change stamp with recipes still to come. Pass it back with `cursor`.
    pub after_id: Option<Uuid>,
    /// True when more changes follow this page. Request them right away.
    pub has_more: bool,
    /// True when the request's cursor couldn't be answered with a delta: it
    /// was absent, predates a restored backup, or came from another
    /// database. The stream then starts over from every active recipe, so
    /// the client must drop everything it has cached before applying this
    /// page (recipes the server lost have no tombstone). Later pages of the
    /// same sync are ordinary deltas.
    pub full_resync: bool,
    /// Version of the shared search-normalization contract
    /// (shared-test-vectors/search-normalization.json) the server was built
    /// with. A client mirroring server search locally must fail the sync when
    /// it does not support this version: matching with a stale contract would
    /// silently drop or add results relative to server search.
    pub normalization_contract_version: u32,
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
    /// The stored ingredients JSON text, exactly as the database holds it — the
    /// exact haystack the server's bare-text search filter matches (JSON keys
    /// and syntax included). Local search must match against this string, not
    /// a re-encoding of `ingredients`, to reproduce server result membership.
    pub ingredient_match_text: String,
    pub instructions: String,
    pub notes: Option<String>,
    /// What `source:` filters match.
    pub source_name: Option<String>,
    /// Estimated calories per serving: what `calories:` filters and the
    /// calorie sort compare. Null when there's no per-serving estimate.
    pub kcal_per_serving: Option<f64>,
    /// The estimate for display, as `RecipeSummary.calories_per_serving`.
    pub calories_per_serving: Option<String>,
    /// The recipe's photos, in order, leaving out deleted ones: what
    /// `photo_size:` and `photo_dim:` filters compare.
    pub photos: Vec<SyncPhoto>,
}

/// The stored facts about one photo that search filters on.
#[derive(Debug, Clone, Serialize, ToSchema, Queryable)]
pub struct SyncPhoto {
    /// Size of the stored file in bytes.
    pub file_size: Option<i32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

type PhotoFacts = HashMap<Uuid, SyncPhoto>;

/// One page of the stream and where the next one starts.
struct Page {
    rows: Vec<RecipeSyncRow>,
    position: (i64, Option<Uuid>),
    has_more: bool,
}

/// The live photos among `ids`.
fn photo_facts(conn: &mut SqliteConnection, ids: Vec<Uuid>) -> QueryResult<PhotoFacts> {
    let mut facts = PhotoFacts::new();
    // Chunked to stay under SQLite's bound-parameter limit.
    for chunk in ids.chunks(500) {
        let rows: Vec<(Uuid, SyncPhoto)> = photos::table
            .filter(photos::id.eq_any(chunk))
            .filter(photos::deleted_at.is_null())
            .select((
                photos::id,
                (photos::file_size, photos::width, photos::height),
            ))
            .load(conn)?;
        facts.extend(rows);
    }
    Ok(facts)
}

impl SyncRecipe {
    fn try_from_row(row: RecipeSyncRow, photos: &PhotoFacts) -> Result<Self, serde_json::Error> {
        let ingredients = serde_json::from_value(row.ingredients)?;
        let photos = row
            .photo_ids
            .iter()
            .flatten()
            .filter_map(|id| photos.get(id).cloned())
            .collect();
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
            ingredient_match_text: row.ingredient_match_text,
            instructions: row.instructions,
            notes: row.notes,
            source_name: row.source_name,
            kcal_per_serving: row.kcal_per_serving,
            calories_per_serving: row.calories_per_serving,
            photos,
        })
    }
}

#[utoipa::path(
    get,
    path = "/api/sync/recipes",
    tag = "sync",
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
) -> Result<impl IntoResponse, ApiError> {
    if !(1..=MAX_SYNC_PAGE_SIZE).contains(&params.limit) {
        return Err(ApiError::invalid_request(format!(
            "limit must be between 1 and {MAX_SYNC_PAGE_SIZE}"
        )));
    }

    let user_id = user.id;

    // One read transaction, so the cursor and the reads come from a single
    // snapshot (see `db::sync_cursor`). Applying a redelivered change is
    // idempotent.
    //
    // The stream is ordered by change stamp (`raw_sql::recipe_sync_seq`),
    // then recipe id, and a page's position is the last (stamp, id) it
    // returned. Every change committed after a page's snapshot takes a stamp
    // above everything that page could see, so it sorts after the client's
    // position whichever page the client is on. Nothing is skipped, and no
    // page has to remember where the sync began.
    let (page, photos, deleted, full_resync) = run_db(&pool, move |conn| {
        conn.transaction(|conn| {
            // Runs first so it establishes the snapshot the reads below use.
            let sync_cursor = db::sync_cursor(conn)?;
            let since = match (params.cursor, params.after_id) {
                (Some(cursor), Some(_)) => sync_cursor.continuing(cursor),
                (cursor, _) => sync_cursor.since(cursor),
            };

            let mut query = current_recipe_versions_for_user!(user_id).into_boxed();
            match (since, params.after_id) {
                (Some(since), Some(after_id)) => {
                    query = query.filter(
                        raw_sql::recipe_sync_seq()
                            .gt(since)
                            .or(raw_sql::recipe_sync_seq()
                                .eq(since)
                                .and(recipes::id.gt(after_id))),
                    );
                }
                (Some(since), None) => {
                    query = query.filter(raw_sql::recipe_sync_seq().ge(since));
                }
                (None, _) => {}
            }

            let mut rows: Vec<RecipeSyncRow> = query
                .select(recipe_sync_select!())
                .order((raw_sql::recipe_sync_seq().asc(), recipes::id.asc()))
                .limit(params.limit + 1)
                .load(conn)?;
            let has_more = rows.len() as i64 > params.limit;
            rows.truncate(params.limit as usize);

            // Bare IDs, so every delta page carries all of them from its
            // position on rather than paging them too.
            let deleted: Vec<Uuid> = match since {
                Some(since) => recipes::table
                    .filter(recipes::user_id.eq(user_id))
                    .filter(recipes::deleted_at.is_not_null())
                    .filter(recipes::deleted_seq.ge(since))
                    .select(recipes::id)
                    .load(conn)?,
                None => Vec::new(),
            };

            let photo_ids = rows
                .iter()
                .flat_map(|row| row.photo_ids.iter().flatten().copied())
                .collect();
            let photos = photo_facts(conn, photo_ids)?;

            // Mid-stream the position is the last row; at the end it is the
            // clock, past every stamp this snapshot could see.
            let position = match rows.last() {
                Some(last) if has_more => (last.sync_seq, Some(last.id)),
                _ => (sync_cursor.value, None),
            };
            Ok::<_, diesel::result::Error>((
                Page {
                    rows,
                    position,
                    has_more,
                },
                photos,
                deleted,
                since.is_none(),
            ))
        })
        .map_err(|e| {
            tracing::error!("Failed to sync recipes: {}", e);
            ApiError::internal("Failed to sync recipes")
        })
    })
    .await?;
    let Page {
        rows,
        position: (cursor, after_id),
        has_more,
    } = page;

    let changes = match rows
        .into_iter()
        .map(|row| SyncRecipe::try_from_row(row, &photos))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(changes) => changes,
        Err(e) => {
            tracing::error!(error = %e, "stored ingredients JSON failed to deserialize during recipe sync");
            return Err(ApiError::internal("Recipe ingredients are corrupt"));
        }
    };

    Ok((
        StatusCode::OK,
        Json(SyncRecipesResponse {
            changes,
            deleted,
            cursor,
            after_id,
            has_more,
            full_resync,
            normalization_contract_version: ramekin_core::search::normalization_contract_version(),
        }),
    ))
}
