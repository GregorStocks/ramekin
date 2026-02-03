use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::NewShoppingListItem;
use crate::schema::shopping_list_items;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::upsert::on_constraint;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

// Type aliases for complex tuple types
type ItemUpdateRow = (String, Option<String>, Option<String>, bool, i32, i32);
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

    let result = conn.transaction(|conn| {
        // 1. Process creates
        let mut created = Vec::with_capacity(request.creates.len());
        for create_req in &request.creates {
            let amount_ref = create_req.amount.as_deref();
            let note_ref = create_req.note.as_deref();
            let source_title_ref = create_req.source_recipe_title.as_deref();

            let new_item = NewShoppingListItem {
                user_id: user.id,
                item: &create_req.item,
                amount: amount_ref,
                note: note_ref,
                source_recipe_id: create_req.source_recipe_id,
                source_recipe_title: source_title_ref,
                is_checked: create_req.is_checked,
                sort_order: create_req.sort_order,
                client_id: Some(create_req.client_id),
            };

            // Use the unique constraint for conflict detection (dedup offline syncs)
            let (server_id, version) = match diesel::insert_into(shopping_list_items::table)
                .values(&new_item)
                .on_conflict(on_constraint("uq_shopping_list_client_id"))
                .do_nothing()
                .returning((shopping_list_items::id, shopping_list_items::version))
                .get_result::<(Uuid, i32)>(conn)
            {
                Ok(result) => result,
                Err(diesel::result::Error::NotFound) => shopping_list_items::table
                    .filter(shopping_list_items::user_id.eq(user.id))
                    .filter(shopping_list_items::client_id.eq(create_req.client_id))
                    .select((shopping_list_items::id, shopping_list_items::version))
                    .first::<(Uuid, i32)>(conn)?,
                Err(e) => return Err(e),
            };

            created.push(SyncCreatedItem {
                client_id: create_req.client_id,
                server_id,
                version,
            });
        }

        // 2. Process updates (with optimistic locking)
        let mut updated = Vec::with_capacity(request.updates.len());
        for update_req in &request.updates {
            // Fetch current state
            let current: Option<ItemUpdateRow> = shopping_list_items::table
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

            let Some((
                current_item,
                current_amount,
                current_note,
                current_checked,
                current_order,
                current_version,
            )) = current
            else {
                updated.push(SyncUpdatedItem {
                    id: update_req.id,
                    version: 0,
                    success: false,
                });
                continue;
            };

            // Check version for conflict
            if current_version != update_req.expected_version {
                // Conflict - server wins, return current version
                updated.push(SyncUpdatedItem {
                    id: update_req.id,
                    version: current_version,
                    success: false,
                });
                continue;
            }

            // Apply update
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
                updated.push(SyncUpdatedItem {
                    id: update_req.id,
                    version: new_version,
                    success: true,
                });
            } else {
                updated.push(SyncUpdatedItem {
                    id: update_req.id,
                    version: current_version,
                    success: false,
                });
            }
        }

        // 3. Process deletes
        let mut deleted_set: HashSet<Uuid> = HashSet::with_capacity(request.deletes.len());
        for delete_id in &request.deletes {
            let updated_rows = diesel::update(
                shopping_list_items::table
                    .filter(shopping_list_items::id.eq(delete_id))
                    .filter(shopping_list_items::user_id.eq(user.id))
                    .filter(shopping_list_items::deleted_at.is_null()),
            )
            .set((
                shopping_list_items::deleted_at.eq(sync_timestamp),
                shopping_list_items::updated_at.eq(sync_timestamp),
                shopping_list_items::version.eq(shopping_list_items::version + 1),
            ))
            .execute(conn)?;

            if updated_rows == 1 {
                deleted_set.insert(*delete_id);
                continue;
            }

            let exists = shopping_list_items::table
                .filter(shopping_list_items::id.eq(delete_id))
                .filter(shopping_list_items::user_id.eq(user.id))
                .select(shopping_list_items::id)
                .first::<Uuid>(conn)
                .optional()?;

            if exists.is_some() {
                deleted_set.insert(*delete_id);
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
        })
    });

    match result {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => {
            tracing::error!("Failed to sync shopping list: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to sync shopping list".to_string(),
                }),
            )
                .into_response()
        }
    }
}
