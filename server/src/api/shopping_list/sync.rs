use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::NewShoppingListItem;
use crate::schema::shopping_list_items;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::upsert::on_constraint;
use ramekin_core::ingredient_categorizer;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

// Type aliases for complex tuple types
type ItemUpdateRow = (String, Option<String>, Option<String>, bool, i32, i32);
type ItemUpdateRowWithId = (Uuid, String, Option<String>, Option<String>, bool, i32, i32);
type ServerChangeRow = (
    Uuid,
    String,
    Option<String>,
    Option<String>,
    Option<Uuid>,
    Option<String>,
    bool,
    i32,
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
    /// Expected version for optimistic locking
    pub expected_version: i32,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SyncRequest {
    /// Last sync timestamp - server will return changes since this time
    pub last_sync_at: Option<DateTime<Utc>>,
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
    /// Computed aisle category for grouping (e.g., "Produce", "Dairy & Eggs")
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
    /// Server-side changes since last_sync_at
    pub server_changes: Vec<SyncServerChange>,
    /// New sync timestamp to use for next sync
    pub sync_timestamp: DateTime<Utc>,
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
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);
    let sync_timestamp = Utc::now();

    let result = conn.transaction::<_, diesel::result::Error, _>(|conn| {
        // 1. Process creates — batch insert, fall back to a single SELECT for conflicts
        let mut created = Vec::with_capacity(request.creates.len());
        if !request.creates.is_empty() {
            let new_items: Vec<NewShoppingListItem> = request
                .creates
                .iter()
                .map(|c| NewShoppingListItem {
                    user_id: user.id,
                    item: &c.item,
                    amount: c.amount.as_deref(),
                    note: c.note.as_deref(),
                    source_recipe_id: c.source_recipe_id,
                    source_recipe_title: c.source_recipe_title.as_deref(),
                    is_checked: c.is_checked,
                    sort_order: c.sort_order,
                    client_id: Some(c.client_id),
                })
                .collect();

            let inserted: Vec<(Option<Uuid>, Uuid, i32)> =
                diesel::insert_into(shopping_list_items::table)
                    .values(&new_items)
                    .on_conflict(on_constraint("uq_shopping_list_client_id"))
                    .do_nothing()
                    .returning((
                        shopping_list_items::client_id,
                        shopping_list_items::id,
                        shopping_list_items::version,
                    ))
                    .get_results(conn)?;

            let mut by_client_id: HashMap<Uuid, (Uuid, i32)> =
                HashMap::with_capacity(request.creates.len());
            for (cid, id, ver) in inserted {
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
                    .filter(shopping_list_items::user_id.eq(user.id))
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
                .filter(shopping_list_items::user_id.eq(user.id))
                .filter(shopping_list_items::deleted_at.is_null())
                .select((
                    shopping_list_items::id,
                    shopping_list_items::item,
                    shopping_list_items::amount,
                    shopping_list_items::note,
                    shopping_list_items::is_checked,
                    shopping_list_items::sort_order,
                    shopping_list_items::version,
                ))
                .load(conn)?;

            let mut current_map: HashMap<Uuid, ItemUpdateRow> =
                HashMap::with_capacity(current_rows.len());
            for (id, item, amount, note, is_checked, sort_order, version) in current_rows {
                current_map.insert(id, (item, amount, note, is_checked, sort_order, version));
            }

            for update_req in &request.updates {
                let Some((
                    current_item,
                    current_amount,
                    current_note,
                    current_checked,
                    current_order,
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
                let new_version = current_version + 1;

                let updated_rows = diesel::update(
                    shopping_list_items::table
                        .filter(shopping_list_items::id.eq(update_req.id))
                        .filter(shopping_list_items::user_id.eq(user.id))
                        .filter(shopping_list_items::deleted_at.is_null())
                        .filter(shopping_list_items::version.eq(update_req.expected_version)),
                )
                .set((
                    shopping_list_items::item.eq(&new_item),
                    shopping_list_items::amount.eq(&new_amount),
                    shopping_list_items::note.eq(&new_note),
                    shopping_list_items::is_checked.eq(new_checked),
                    shopping_list_items::sort_order.eq(new_order),
                    shopping_list_items::version.eq(new_version),
                    shopping_list_items::updated_at.eq(sync_timestamp),
                ))
                .execute(conn)?;

                if updated_rows == 1 {
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
                        .filter(shopping_list_items::user_id.eq(user.id))
                        .filter(shopping_list_items::deleted_at.is_null())
                        .select((
                            shopping_list_items::item,
                            shopping_list_items::amount,
                            shopping_list_items::note,
                            shopping_list_items::is_checked,
                            shopping_list_items::sort_order,
                            shopping_list_items::version,
                        ))
                        .first(conn)
                        .optional()?;

                    match fresh {
                        Some(row) => {
                            let fresh_version = row.5;
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
                    .filter(shopping_list_items::user_id.eq(user.id))
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
                    .filter(shopping_list_items::user_id.eq(user.id))
                    .select(shopping_list_items::id)
                    .load(conn)?;
                deleted_set.extend(already_deleted);
            }
        }

        // 4. Get server changes since last_sync_at
        let server_changes: Vec<SyncServerChange> = if let Some(last_sync) = request.last_sync_at {
            let rows: Vec<ServerChangeRow> = shopping_list_items::table
                .filter(shopping_list_items::user_id.eq(user.id))
                .filter(shopping_list_items::deleted_at.is_null())
                .filter(shopping_list_items::updated_at.gt(last_sync))
                .select((
                    shopping_list_items::id,
                    shopping_list_items::item,
                    shopping_list_items::amount,
                    shopping_list_items::note,
                    shopping_list_items::source_recipe_id,
                    shopping_list_items::source_recipe_title,
                    shopping_list_items::is_checked,
                    shopping_list_items::sort_order,
                    shopping_list_items::version,
                    shopping_list_items::updated_at,
                ))
                .load(conn)?;

            rows.into_iter()
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
                        version,
                        updated_at,
                    )| {
                        let category = ingredient_categorizer::categorize(&item).to_string();
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
                            category,
                        }
                    },
                )
                .collect()
        } else {
            // No last_sync_at means first sync - return all items
            let rows: Vec<ServerChangeRow> = shopping_list_items::table
                .filter(shopping_list_items::user_id.eq(user.id))
                .filter(shopping_list_items::deleted_at.is_null())
                .select((
                    shopping_list_items::id,
                    shopping_list_items::item,
                    shopping_list_items::amount,
                    shopping_list_items::note,
                    shopping_list_items::source_recipe_id,
                    shopping_list_items::source_recipe_title,
                    shopping_list_items::is_checked,
                    shopping_list_items::sort_order,
                    shopping_list_items::version,
                    shopping_list_items::updated_at,
                ))
                .load(conn)?;

            rows.into_iter()
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
                        version,
                        updated_at,
                    )| {
                        let category = ingredient_categorizer::categorize(&item).to_string();
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
                            category,
                        }
                    },
                )
                .collect()
        };

        if let Some(last_sync) = request.last_sync_at {
            let deleted_rows: Vec<Uuid> = shopping_list_items::table
                .filter(shopping_list_items::user_id.eq(user.id))
                .filter(shopping_list_items::deleted_at.gt(last_sync))
                .select(shopping_list_items::id)
                .load(conn)?;

            deleted_set.extend(deleted_rows);
        }

        Ok(SyncResponse {
            created,
            updated,
            deleted: deleted_set.into_iter().collect(),
            server_changes,
            sync_timestamp,
            category_order: super::list::category_order(),
        })
    });

    match result {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => {
            tracing::error!("Failed to sync shopping list: {}", e);
            ApiError::internal("Failed to sync shopping list").into_response()
        }
    }
}
