use crate::api::{run_db_locked, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::{self, DbPool};
use crate::ids::Uuid;
use crate::models::NewShoppingListItem;
use crate::schema::shopping_list_items;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_with::rust::double_option;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use utoipa::ToSchema;

// Type aliases for complex tuple types
type ItemUpdateRow = (
    String,
    Option<String>,
    Option<String>,
    bool,
    i32,
    Option<String>,
    i32,
);
type ItemUpdateRowWithId = (
    Uuid,
    String,
    Option<String>,
    Option<String>,
    bool,
    i32,
    Option<String>,
    i32,
);
type ServerChangeRow = (
    Uuid,
    String,
    Option<String>,
    Option<String>,
    Option<Uuid>,
    Option<String>,
    bool,
    i32,
    Option<String>,
    Option<String>,
    i32,
    DateTime<Utc>,
);

/// Request to create an item during sync (created offline)
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SyncCreateItem {
    pub client_id: Uuid,
    pub item: String,
    pub amount: Option<String>,
    pub note: Option<String>,
    pub source_recipe_id: Option<Uuid>,
    pub source_recipe_title: Option<String>,
    pub is_checked: bool,
    pub sort_order: i32,
    pub category_override: Option<String>,
}

/// Request to update an item during sync (modified offline)
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SyncUpdateItem {
    pub id: Uuid,
    pub item: Option<String>,
    pub amount: Option<String>,
    pub note: Option<String>,
    pub is_checked: Option<bool>,
    pub sort_order: Option<i32>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<String>)]
    pub category_override: Option<Option<String>>,
    pub clear_category_override: Option<bool>,
    /// Expected version for optimistic locking
    pub expected_version: i32,
}

/// Which server changes a sync returns.
#[derive(Debug, Clone, Copy)]
enum ChangesSince {
    All,
    Cursor(i64),
    /// A cursor this database can't answer with a delta: every item, and
    /// every deletion, as if from the start.
    Restart,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SyncRequest {
    /// `cursor` from the previous sync's response. Server returns changes at
    /// or after it; without one, it returns every item.
    pub cursor: Option<i64>,
    /// Items created offline
    #[serde(default)]
    pub creates: Vec<SyncCreateItem>,
    /// Items updated offline
    #[serde(default)]
    pub updates: Vec<SyncUpdateItem>,
    /// IDs of items deleted offline
    #[serde(default)]
    pub deletes: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncCreatedItem {
    pub client_id: Uuid,
    pub server_id: Uuid,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncUpdatedItem {
    pub id: Uuid,
    pub version: i32,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncServerChange {
    pub id: Uuid,
    pub item: String,
    pub amount: Option<String>,
    pub note: Option<String>,
    pub source_recipe_id: Option<Uuid>,
    pub source_recipe_title: Option<String>,
    pub is_checked: bool,
    pub sort_order: i32,
    pub version: i32,
    pub updated_at: DateTime<Utc>,
    /// User-selected category override; when set, it wins over computed category.
    pub category_override: Option<String>,
    /// Category computed from the item name before applying any override.
    pub computed_category: String,
    /// Aisle category for grouping (override when set, otherwise computed).
    pub category: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncResponse {
    /// Items that were created (maps client_id to server_id)
    pub created: Vec<SyncCreatedItem>,
    /// Items that were updated (with success status)
    pub updated: Vec<SyncUpdatedItem>,
    /// IDs of items that were deleted
    pub deleted: Vec<Uuid>,
    /// Server-side changes since the request's cursor
    pub server_changes: Vec<SyncServerChange>,
    /// Server time when this sync started. Deprecated as a cursor: pass
    /// `cursor` instead.
    pub sync_timestamp: DateTime<Utc>,
    /// Snapshot watermark to pass as `cursor` on the next sync. Changes may be
    /// redelivered across syncs, but none can be skipped.
    pub cursor: i64,
    /// True when `server_changes` holds every live item rather than changes
    /// since the request's cursor: no cursor was sent, or it predates a
    /// restored backup or import, or came from another database. The client
    /// must then drop synced cached items missing from `server_changes` — an
    /// item rolled back by a restore is in neither `server_changes` nor
    /// `deleted`.
    pub full_resync: bool,
    /// Canonical category display order for grouping items; every item's
    /// `category` is guaranteed to appear in this list.
    pub category_order: Vec<String>,
}

#[utoipa::path(
    post,
    path = "/api/shopping-list/sync",
    tag = "shopping_list",
    request_body = SyncRequest,
    responses(
        (status = 200, description = "Sync completed", body = SyncResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn sync_items(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<SyncRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if request.creates.iter().any(|item| {
        item.category_override
            .as_deref()
            .is_some_and(|category| !super::list::is_valid_category(category))
    }) || request.updates.iter().any(|item| {
        item.category_override
            .as_ref()
            .and_then(|category| category.as_deref())
            .is_some_and(|category| !super::list::is_valid_category(category))
    }) {
        return Err(ApiError::invalid_request("Invalid category override"));
    }
    if request.updates.iter().any(|item| {
        item.clear_category_override == Some(true)
            && matches!(item.category_override, Some(Some(_)))
    }) {
        return Err(ApiError::invalid_request(
            "Conflicting category override fields",
        ));
    }

    let user_id = user.id;
    let sync_timestamp = Utc::now();
    let since = match request.cursor {
        Some(cursor) => ChangesSince::Cursor(cursor),
        None => ChangesSince::All,
    };

    let response = run_db_locked(&pool, move |conn| {
        db::write::<_, diesel::result::Error, _>(conn, |conn| {
            // Read before this sync's own writes, which therefore carry stamps
            // at or above it and come back on the next sync. Clients apply
            // redelivered changes idempotently by version.
            let sync_cursor = db::sync_cursor(conn)?;
            let cursor = sync_cursor.value;
            // A cursor this database can't answer with a delta (see
            // `SyncCursor::since`) gets everything, deletions included, as if
            // from the start.
            let since = match since {
                ChangesSince::Cursor(since) => match sync_cursor.since(Some(since)) {
                    Some(since) => ChangesSince::Cursor(since),
                    None => ChangesSince::Restart,
                },
                other => other,
            };
            let full_resync = !matches!(since, ChangesSince::Cursor(_));

            // 1. Process creates — batch insert, fall back to a single SELECT for conflicts
            let mut created = Vec::with_capacity(request.creates.len());
            // Names this sync actually wrote, queued for resolution below.
            let mut written_items: Vec<String> = Vec::new();
            // Items this sync wrote, whose categories are stored below.
            let mut categorized: Vec<(Uuid, String)> = Vec::new();
            if !request.creates.is_empty() {
                let new_items: Vec<NewShoppingListItem> = request
                    .creates
                    .iter()
                    .map(|c| NewShoppingListItem {
                        id: Uuid::now_v7(),
                        user_id,
                        item: &c.item,
                        amount: c.amount.as_deref(),
                        note: c.note.as_deref(),
                        source_recipe_id: c.source_recipe_id,
                        source_recipe_title: c.source_recipe_title.as_deref(),
                        is_checked: c.is_checked,
                        sort_order: c.sort_order,
                        category_override: c.category_override.as_deref(),
                        client_id: Some(c.client_id),
                    })
                    .collect();

                let inserted: Vec<(Option<Uuid>, Uuid, i32, String)> =
                    diesel::insert_into(shopping_list_items::table)
                        .values(&new_items)
                        .on_conflict((shopping_list_items::user_id, shopping_list_items::client_id))
                        .do_nothing()
                        .returning((
                            shopping_list_items::client_id,
                            shopping_list_items::id,
                            shopping_list_items::version,
                            shopping_list_items::item,
                        ))
                        .get_results(conn)?;

                let mut by_client_id: HashMap<Uuid, (Uuid, i32)> =
                    HashMap::with_capacity(request.creates.len());
                for (cid, id, ver, item) in inserted {
                    written_items.push(item.clone());
                    categorized.push((id, item));
                    if let Some(cid) = cid {
                        by_client_id.insert(cid, (id, ver));
                    }
                }

                let missing_client_ids: Vec<Uuid> = request
                    .creates
                    .iter()
                    .map(|c| c.client_id)
                    .filter(|cid| !by_client_id.contains_key(cid))
                    .collect();

                if !missing_client_ids.is_empty() {
                    let existing: Vec<(Option<Uuid>, Uuid, i32)> = shopping_list_items::table
                        .filter(shopping_list_items::user_id.eq(user_id))
                        .filter(shopping_list_items::client_id.eq_any(&missing_client_ids))
                        .select((
                            shopping_list_items::client_id,
                            shopping_list_items::id,
                            shopping_list_items::version,
                        ))
                        .load(conn)?;
                    for (cid, id, ver) in existing {
                        if let Some(cid) = cid {
                            by_client_id.insert(cid, (id, ver));
                        }
                    }
                }

                for create_req in &request.creates {
                    if let Some((server_id, version)) = by_client_id.get(&create_req.client_id) {
                        created.push(SyncCreatedItem {
                            client_id: create_req.client_id,
                            server_id: *server_id,
                            version: *version,
                        });
                    }
                }
            }

            // 2. Process updates — prefetch all current states in one query, then apply each
            let mut updated = Vec::with_capacity(request.updates.len());
            if !request.updates.is_empty() {
                let update_ids: Vec<Uuid> = request.updates.iter().map(|u| u.id).collect();
                let current_rows: Vec<ItemUpdateRowWithId> = shopping_list_items::table
                    .filter(shopping_list_items::id.eq_any(&update_ids))
                    .filter(shopping_list_items::user_id.eq(user_id))
                    .filter(shopping_list_items::deleted_at.is_null())
                    .select((
                        shopping_list_items::id,
                        shopping_list_items::item,
                        shopping_list_items::amount,
                        shopping_list_items::note,
                        shopping_list_items::is_checked,
                        shopping_list_items::sort_order,
                        shopping_list_items::category_override,
                        shopping_list_items::version,
                    ))
                    .load(conn)?;

                let mut current_map: HashMap<Uuid, ItemUpdateRow> =
                    HashMap::with_capacity(current_rows.len());
                for (id, item, amount, note, is_checked, sort_order, category_override, version) in
                    current_rows
                {
                    current_map.insert(
                        id,
                        (
                            item,
                            amount,
                            note,
                            is_checked,
                            sort_order,
                            category_override,
                            version,
                        ),
                    );
                }

                for update_req in &request.updates {
                    let Some((
                        current_item,
                        current_amount,
                        current_note,
                        current_checked,
                        current_order,
                        current_category_override,
                        current_version,
                    )) = current_map.get(&update_req.id).cloned()
                    else {
                        updated.push(SyncUpdatedItem {
                            id: update_req.id,
                            version: 0,
                            success: false,
                        });
                        continue;
                    };

                    if current_version != update_req.expected_version {
                        updated.push(SyncUpdatedItem {
                            id: update_req.id,
                            version: current_version,
                            success: false,
                        });
                        continue;
                    }

                    let new_item = update_req.item.clone().unwrap_or(current_item);
                    let new_amount = update_req.amount.clone().or(current_amount);
                    let new_note = update_req.note.clone().or(current_note);
                    let new_checked = update_req.is_checked.unwrap_or(current_checked);
                    let new_order = update_req.sort_order.unwrap_or(current_order);
                    let new_category_override = if update_req.clear_category_override == Some(true)
                    {
                        None
                    } else {
                        update_req
                            .category_override
                            .clone()
                            .unwrap_or(current_category_override)
                    };
                    let new_version = current_version + 1;

                    let updated_rows = diesel::update(
                        shopping_list_items::table
                            .filter(shopping_list_items::id.eq(update_req.id))
                            .filter(shopping_list_items::user_id.eq(user_id))
                            .filter(shopping_list_items::deleted_at.is_null())
                            .filter(shopping_list_items::version.eq(update_req.expected_version)),
                    )
                    .set((
                        shopping_list_items::item.eq(&new_item),
                        shopping_list_items::amount.eq(&new_amount),
                        shopping_list_items::note.eq(&new_note),
                        shopping_list_items::is_checked.eq(new_checked),
                        shopping_list_items::sort_order.eq(new_order),
                        shopping_list_items::category_override.eq(&new_category_override),
                        shopping_list_items::version.eq(new_version),
                        shopping_list_items::updated_at.eq(sync_timestamp),
                    ))
                    .execute(conn)?;

                    if updated_rows == 1 {
                        if update_req.item.is_some() {
                            written_items.push(new_item.clone());
                        }
                        categorized.push((update_req.id, new_item.clone()));
                        // Write-back so a later update for the same id in this batch sees
                        // the new state instead of the stale prefetch.
                        current_map.insert(
                            update_req.id,
                            (
                                new_item,
                                new_amount,
                                new_note,
                                new_checked,
                                new_order,
                                new_category_override,
                                new_version,
                            ),
                        );
                        updated.push(SyncUpdatedItem {
                            id: update_req.id,
                            version: new_version,
                            success: true,
                        });
                    } else {
                        // Race: our prefetched version matched expected_version but the
                        // UPDATE matched zero rows, meaning another writer changed the row
                        // between our prefetch and our update. Re-read so a later update
                        // for the same id in this batch sees the true state and the
                        // response reports the true version.
                        let fresh: Option<ItemUpdateRow> = shopping_list_items::table
                            .filter(shopping_list_items::id.eq(update_req.id))
                            .filter(shopping_list_items::user_id.eq(user_id))
                            .filter(shopping_list_items::deleted_at.is_null())
                            .select((
                                shopping_list_items::item,
                                shopping_list_items::amount,
                                shopping_list_items::note,
                                shopping_list_items::is_checked,
                                shopping_list_items::sort_order,
                                shopping_list_items::category_override,
                                shopping_list_items::version,
                            ))
                            .first(conn)
                            .optional()?;

                        match fresh {
                            Some(row) => {
                                let fresh_version = row.6;
                                current_map.insert(update_req.id, row);
                                updated.push(SyncUpdatedItem {
                                    id: update_req.id,
                                    version: fresh_version,
                                    success: false,
                                });
                            }
                            None => {
                                current_map.remove(&update_req.id);
                                updated.push(SyncUpdatedItem {
                                    id: update_req.id,
                                    version: 0,
                                    success: false,
                                });
                            }
                        }
                    }
                }
            }

            // 3. Process deletes — one batch UPDATE, then a single SELECT for any that didn't match
            let mut deleted_set: HashSet<Uuid> = HashSet::with_capacity(request.deletes.len());
            if !request.deletes.is_empty() {
                let newly_deleted: Vec<Uuid> = diesel::update(
                    shopping_list_items::table
                        .filter(shopping_list_items::id.eq_any(&request.deletes))
                        .filter(shopping_list_items::user_id.eq(user_id))
                        .filter(shopping_list_items::deleted_at.is_null()),
                )
                .set((
                    shopping_list_items::deleted_at.eq(sync_timestamp),
                    shopping_list_items::updated_at.eq(sync_timestamp),
                    shopping_list_items::version.eq(shopping_list_items::version + 1),
                ))
                .returning(shopping_list_items::id)
                .get_results(conn)?;

                deleted_set.extend(newly_deleted.iter().copied());

                let remaining: Vec<Uuid> = request
                    .deletes
                    .iter()
                    .copied()
                    .filter(|id| !deleted_set.contains(id))
                    .collect();

                if !remaining.is_empty() {
                    let already_deleted: Vec<Uuid> = shopping_list_items::table
                        .filter(shopping_list_items::id.eq_any(&remaining))
                        .filter(shopping_list_items::user_id.eq(user_id))
                        .select(shopping_list_items::id)
                        .load(conn)?;
                    deleted_set.extend(already_deleted);
                }
            }

            // A rejected update's item comes back in the changes even if it
            // changed before the client's cursor, so the client can take the
            // server's copy instead of resending its stale one.
            let conflicted: Vec<Uuid> = updated
                .iter()
                .filter(|u| !u.success && u.version > 0)
                .map(|u| u.id)
                .collect();

            // Queue the names this sync wrote, then store its items' categories
            // (in that order; see `store_categories`), before reading changes.
            crate::ingredient_names::enqueue_items(conn, written_items.iter().map(String::as_str))?;
            let categorized: Vec<(Uuid, &str)> = categorized
                .iter()
                .map(|(id, item)| (*id, item.as_str()))
                .collect();
            super::list::store_categories(conn, &categorized)?;

            // 4. Get server changes since the client's last sync
            let mut changes_query = shopping_list_items::table
                .filter(shopping_list_items::user_id.eq(user_id))
                .filter(shopping_list_items::deleted_at.is_null())
                .into_boxed();
            let deleted_items = shopping_list_items::table
                .filter(shopping_list_items::user_id.eq(user_id))
                .filter(shopping_list_items::deleted_at.is_not_null())
                .into_boxed();
            let deleted_query = match since {
                // No cursor means first sync - return all items and no deletes
                ChangesSince::All => None,
                ChangesSince::Restart => Some(deleted_items),
                ChangesSince::Cursor(since) => {
                    changes_query = changes_query.filter(
                        shopping_list_items::change_seq
                            .ge(since)
                            .or(shopping_list_items::id.eq_any(conflicted)),
                    );
                    Some(deleted_items.filter(shopping_list_items::change_seq.ge(since)))
                }
            };

            let rows: Vec<ServerChangeRow> = changes_query
                .select((
                    shopping_list_items::id,
                    shopping_list_items::item,
                    shopping_list_items::amount,
                    shopping_list_items::note,
                    shopping_list_items::source_recipe_id,
                    shopping_list_items::source_recipe_title,
                    shopping_list_items::is_checked,
                    shopping_list_items::sort_order,
                    shopping_list_items::category_override,
                    shopping_list_items::computed_category,
                    shopping_list_items::version,
                    shopping_list_items::updated_at,
                ))
                .load(conn)?;

            let server_changes: Vec<SyncServerChange> = rows
                .into_iter()
                .map(
                    |(
                        id,
                        item,
                        amount,
                        note,
                        source_recipe_id,
                        source_recipe_title,
                        is_checked,
                        sort_order,
                        category_override,
                        computed_category,
                        version,
                        updated_at,
                    )| {
                        // Every write stores one, and startup fills in older items.
                        let computed_category =
                            computed_category.unwrap_or_else(|| "Other".to_string());
                        let category = super::list::item_category(
                            &computed_category,
                            category_override.as_deref(),
                        );
                        SyncServerChange {
                            id,
                            item,
                            amount,
                            note,
                            source_recipe_id,
                            source_recipe_title,
                            is_checked,
                            sort_order,
                            version,
                            updated_at,
                            category_override,
                            computed_category,
                            category,
                        }
                    },
                )
                .collect();

            if let Some(deleted_query) = deleted_query {
                let deleted_rows: Vec<Uuid> =
                    deleted_query.select(shopping_list_items::id).load(conn)?;
                deleted_set.extend(deleted_rows);
            }

            Ok(SyncResponse {
                created,
                updated,
                deleted: deleted_set.into_iter().collect(),
                server_changes,
                sync_timestamp,
                cursor,
                full_resync,
                category_order: super::list::category_order(),
            })
        })
        .map_err(|e| {
            tracing::error!("Failed to sync shopping list: {}", e);
            ApiError::internal("Failed to sync shopping list")
        })
    })
    .await?;

    crate::ingredient_names::wake();

    Ok((StatusCode::OK, Json(response)))
}
