use super::{patch_text, refused, text_or_none, with_refused, OpKind, OpResult, Target, Window};
use crate::api::{run_db_write, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::ids::Uuid;
use crate::models::NewShoppingListItem;
use crate::schema::shopping_list_items;
use crate::shopping_list::{category_order, is_valid_category, item_category, store_categories};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::Utc;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// An item's writable fields. On `create`, `item` is required and the rest
/// default (unchecked, sorted last). On `update`, only the fields present
/// change. `amount`, `note` and `category_override` are cleared with "".
/// The source recipe is set on create and never changes.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct ShoppingListFields {
    pub item: Option<String>,
    pub amount: Option<String>,
    pub note: Option<String>,
    pub source_recipe_id: Option<Uuid>,
    pub source_recipe_title: Option<String>,
    pub is_checked: Option<bool>,
    pub sort_order: Option<i32>,
    /// User-selected aisle category; wins over the computed one.
    pub category_override: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ShoppingListOp {
    pub op: OpKind,
    /// The item's id. A create chooses it.
    pub id: Uuid,
    #[serde(default)]
    pub fields: ShoppingListFields,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SyncShoppingListRequest {
    /// `cursor` from the previous sync. Absent means a full sync.
    pub cursor: Option<i64>,
    /// Writes to apply, in order, before reading changes.
    #[serde(default)]
    pub ops: Vec<ShoppingListOp>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncShoppingListItem {
    pub id: Uuid,
    pub item: String,
    pub amount: Option<String>,
    pub note: Option<String>,
    pub source_recipe_id: Option<Uuid>,
    pub source_recipe_title: Option<String>,
    pub is_checked: bool,
    pub sort_order: i32,
    /// User-selected category override; when set, it wins over computed category.
    pub category_override: Option<String>,
    /// Category computed from the item name before applying any override.
    pub computed_category: String,
    /// Aisle category for grouping (override when set, otherwise computed).
    pub category: String,
    /// Sync stamp of the row's last change. A cached copy with a higher one
    /// is newer than this row.
    pub change_seq: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncShoppingListResponse {
    /// One per request op, in order.
    pub results: Vec<OpResult>,
    /// Live items changed at or after the request's `cursor`, this request's
    /// own writes included; every live item on a full resync.
    pub changes: Vec<SyncShoppingListItem>,
    /// Item IDs deleted at or after `cursor`, and every refused op's item
    /// that isn't live. Only the latter on a full resync.
    pub deleted: Vec<Uuid>,
    /// Pass as `cursor` on the next sync.
    pub cursor: i64,
    /// True when `changes` holds every live item: no cursor was sent, or it
    /// predates a restored backup, or came from another database. Drop
    /// cached items it omits.
    pub full_resync: bool,
    /// Canonical category display order for grouping items; every item's
    /// `category` is guaranteed to appear in this list.
    pub category_order: Vec<String>,
}

type ItemRow = (
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
    i64,
);

#[utoipa::path(
    post,
    path = "/api/sync/shopping-list",
    tag = "sync",
    request_body = SyncShoppingListRequest,
    responses(
        (status = 200, description = "Ops applied; changes since the cursor", body = SyncShoppingListResponse),
        (status = 400, description = "A malformed op", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn sync_shopping_list(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<SyncShoppingListRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate(&request.ops)?;
    let user_id = user.id;

    let response = run_db_write(&pool, move |conn| {
        let now = Utc::now();
        let mut next_sort_order = shopping_list_items::table
            .filter(shopping_list_items::user_id.eq(user_id))
            .filter(shopping_list_items::deleted_at.is_null())
            .select(diesel::dsl::max(shopping_list_items::sort_order))
            .first::<Option<i32>>(conn)?
            .unwrap_or(0)
            + 1;
        let mut results = Vec::with_capacity(request.ops.len());
        // Names this sync wrote, queued for resolution below.
        let mut renamed: Vec<String> = Vec::new();
        // Items this sync wrote, whose categories are stored below.
        let mut written: Vec<(Uuid, String)> = Vec::new();

        let ids: Vec<Uuid> = request.ops.iter().map(|op| op.id).collect();
        for op in request.ops {
            let owner: Option<(Uuid, bool)> = shopping_list_items::table
                .find(op.id)
                .select((
                    shopping_list_items::user_id,
                    shopping_list_items::deleted_at.is_null(),
                ))
                .first(conn)
                .optional()?;
            let target = Target::of(owner, user_id);
            let live = target == (Target::Mine { live: true });

            let result = match op.op {
                OpKind::Create => match target.existing_create() {
                    Some(result) => result,
                    None => {
                        let item = create(conn, user_id, op.id, op.fields, &mut next_sort_order)?;
                        renamed.push(item.clone());
                        written.push((op.id, item));
                        OpResult::Applied
                    }
                },
                OpKind::Update if !live => OpResult::Gone,
                OpKind::Update => {
                    let fields = op.fields;
                    let (item, amount, note, is_checked, sort_order, category_override): (
                        String,
                        Option<String>,
                        Option<String>,
                        bool,
                        i32,
                        Option<String>,
                    ) = shopping_list_items::table
                        .find(op.id)
                        .select((
                            shopping_list_items::item,
                            shopping_list_items::amount,
                            shopping_list_items::note,
                            shopping_list_items::is_checked,
                            shopping_list_items::sort_order,
                            shopping_list_items::category_override,
                        ))
                        .first(conn)?;
                    if let Some(item) = &fields.item {
                        renamed.push(item.clone());
                    }
                    let item = fields.item.unwrap_or(item);
                    diesel::update(shopping_list_items::table.find(op.id))
                        .set((
                            shopping_list_items::item.eq(&item),
                            shopping_list_items::amount.eq(patch_text(fields.amount, amount)),
                            shopping_list_items::note.eq(patch_text(fields.note, note)),
                            shopping_list_items::is_checked
                                .eq(fields.is_checked.unwrap_or(is_checked)),
                            shopping_list_items::sort_order
                                .eq(fields.sort_order.unwrap_or(sort_order)),
                            shopping_list_items::category_override
                                .eq(patch_text(fields.category_override, category_override)),
                            shopping_list_items::updated_at.eq(now),
                        ))
                        .execute(conn)?;
                    written.push((op.id, item));
                    OpResult::Applied
                }
                OpKind::Delete => {
                    if live {
                        diesel::update(shopping_list_items::table.find(op.id))
                            .set((
                                shopping_list_items::deleted_at.eq(now),
                                shopping_list_items::updated_at.eq(now),
                            ))
                            .execute(conn)?;
                    }
                    target.delete()
                }
            };
            results.push(result);
        }

        // Queue the names this sync wrote, then store its items' categories
        // (in that order; see `store_categories`), before reading changes.
        crate::ingredient_names::enqueue_items(conn, renamed.iter().map(String::as_str))?;
        let written: Vec<(Uuid, &str)> = written
            .iter()
            .map(|(id, item)| (*id, item.as_str()))
            .collect();
        store_categories(conn, &written)?;

        // After the writes, so they come back as changes now rather than on
        // the next sync. The write lock keeps anything else from committing
        // in between.
        let window = Window::read(conn, request.cursor)?;
        let refused = refused(&ids, &results);

        let mut changed = shopping_list_items::table
            .filter(shopping_list_items::user_id.eq(user_id))
            .filter(shopping_list_items::deleted_at.is_null())
            .into_boxed();
        if let Some(since) = window.since {
            changed = changed.filter(
                shopping_list_items::change_seq
                    .ge(since)
                    .or(shopping_list_items::id.eq_any(&refused)),
            );
        }
        let changes: Vec<SyncShoppingListItem> = changed
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
                shopping_list_items::change_seq,
            ))
            .order((
                shopping_list_items::sort_order.asc(),
                shopping_list_items::id.asc(),
            ))
            .load::<ItemRow>(conn)?
            .into_iter()
            .map(sync_item)
            .collect();

        let deleted = match window.since {
            Some(since) => shopping_list_items::table
                .filter(shopping_list_items::user_id.eq(user_id))
                .filter(shopping_list_items::deleted_at.is_not_null())
                .filter(shopping_list_items::change_seq.ge(since))
                .select(shopping_list_items::id)
                .load(conn)?,
            None => Vec::new(),
        };
        let live: Vec<Uuid> = changes.iter().map(|item| item.id).collect();
        let deleted = with_refused(deleted, &refused, &live);

        Ok(SyncShoppingListResponse {
            results,
            changes,
            deleted,
            cursor: window.cursor,
            full_resync: window.full_resync(),
            category_order: category_order(),
        })
    })
    .await?;

    crate::ingredient_names::wake();

    Ok((StatusCode::OK, Json(response)))
}

/// Insert a new item, sorted last unless the op says where, and return its
/// name.
fn create(
    conn: &mut SqliteConnection,
    user_id: Uuid,
    id: Uuid,
    fields: ShoppingListFields,
    next_sort_order: &mut i32,
) -> QueryResult<String> {
    let item = fields.item.unwrap_or_default();
    let sort_order = fields.sort_order.unwrap_or(*next_sort_order);
    *next_sort_order = (*next_sort_order).max(sort_order + 1);
    let amount = text_or_none(fields.amount);
    let note = text_or_none(fields.note);
    let category_override = text_or_none(fields.category_override);
    diesel::insert_into(shopping_list_items::table)
        .values(NewShoppingListItem {
            id,
            user_id,
            item: &item,
            amount: amount.as_deref(),
            note: note.as_deref(),
            source_recipe_id: fields.source_recipe_id,
            source_recipe_title: fields.source_recipe_title.as_deref(),
            is_checked: fields.is_checked.unwrap_or(false),
            sort_order,
            category_override: category_override.as_deref(),
        })
        .execute(conn)?;
    Ok(item)
}

/// Reject a request whose ops can never apply, before writing any of them.
fn validate(ops: &[ShoppingListOp]) -> Result<(), ApiError> {
    for op in ops {
        let fields = &op.fields;
        if op.op == OpKind::Create && fields.item.is_none() {
            return Err(ApiError::invalid_request("A create needs an item"));
        }
        if fields
            .item
            .as_deref()
            .is_some_and(|item| item.trim().is_empty())
        {
            return Err(ApiError::invalid_request("Item can't be blank"));
        }
        if fields
            .category_override
            .as_deref()
            .is_some_and(|category| !category.is_empty() && !is_valid_category(category))
        {
            return Err(ApiError::invalid_request("Invalid category override"));
        }
    }
    Ok(())
}

fn sync_item(
    (
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
        change_seq,
    ): ItemRow,
) -> SyncShoppingListItem {
    // Every write stores one, and startup fills in older items.
    let computed_category = computed_category.unwrap_or_else(|| "Other".to_string());
    let category = item_category(&computed_category, category_override.as_deref());
    SyncShoppingListItem {
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
        category,
        change_seq,
    }
}
