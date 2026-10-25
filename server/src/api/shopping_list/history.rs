//! The shopping list's history: every sync (and undo) that changed the list
//! is one entry, holding each changed item's fields before and after, so
//! clients can show it and undo any entry (`undo.rs`).

use crate::api::sync::OpKind;
use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::ids::Uuid;
use crate::schema::{shopping_list_history, shopping_list_history_events, shopping_list_items};
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

const DEFAULT_HISTORY_LIMIT: i64 = 50;
const MAX_HISTORY_LIMIT: i64 = 200;

/// An item's user-editable fields, as history stores them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemFields {
    pub item: String,
    pub amount: Option<String>,
    pub note: Option<String>,
    pub is_checked: bool,
    pub sort_order: i32,
    pub category_override: Option<String>,
}

/// An item row, deleted or not.
pub struct ItemRow {
    pub user_id: Uuid,
    pub live: bool,
    pub fields: ItemFields,
}

/// One op's change to one item. `before` and `after` are None when the
/// item wasn't live.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub item_id: Uuid,
    pub op: OpKind,
    pub before: Option<ItemFields>,
    pub after: Option<ItemFields>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ShoppingListHistoryEntry {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    /// What the entry changed, e.g. "Checked milk; deleted eggs and bread".
    pub summary: String,
    /// The entry this one undid, when it's an undo.
    pub undoes_id: Option<Uuid>,
    /// The entry that undid this one. Only an entry without one can be undone.
    pub undone_by_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ShoppingListHistoryResponse {
    /// Newest first.
    pub entries: Vec<ShoppingListHistoryEntry>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ShoppingListHistoryParams {
    /// Most entries to return, between 1 and 200. Defaults to 50.
    pub limit: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/shopping-list/history",
    tag = "shopping_list",
    params(ShoppingListHistoryParams),
    responses(
        (status = 200, description = "The latest history entries", body = ShoppingListHistoryResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_shopping_list_history(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Query(params): Query<ShoppingListHistoryParams>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;
    let limit = params
        .limit
        .unwrap_or(DEFAULT_HISTORY_LIMIT)
        .clamp(1, MAX_HISTORY_LIMIT);

    let entries = run_db(&pool, move |conn| {
        let rows: Vec<EntryRow> = shopping_list_history::table
            .filter(shopping_list_history::user_id.eq(user_id))
            .select(ENTRY_COLUMNS)
            .order(shopping_list_history::id.desc())
            .limit(limit)
            .load(conn)?;
        Ok(entries(conn, rows)?)
    })
    .await?;

    Ok((
        StatusCode::OK,
        Json(ShoppingListHistoryResponse { entries }),
    ))
}

type EntryRow = (Uuid, DateTime<Utc>, Option<Uuid>, Option<Uuid>);

/// history_id, item_id, op, before, after.
type EventRow = (
    Uuid,
    Uuid,
    String,
    Option<serde_json::Value>,
    Option<serde_json::Value>,
);

/// user_id, live, then the item's fields.
type ItemColumns = (
    Uuid,
    bool,
    String,
    Option<String>,
    Option<String>,
    bool,
    i32,
    Option<String>,
);

const ENTRY_COLUMNS: (
    shopping_list_history::id,
    shopping_list_history::created_at,
    shopping_list_history::undoes_id,
    shopping_list_history::undone_by_id,
) = (
    shopping_list_history::id,
    shopping_list_history::created_at,
    shopping_list_history::undoes_id,
    shopping_list_history::undone_by_id,
);

/// The user's entry `id`, if there is one.
pub fn load_entry(
    conn: &mut SqliteConnection,
    user_id: Uuid,
    id: Uuid,
) -> QueryResult<Option<ShoppingListHistoryEntry>> {
    let row: Option<EntryRow> = shopping_list_history::table
        .filter(shopping_list_history::id.eq(id))
        .filter(shopping_list_history::user_id.eq(user_id))
        .select(ENTRY_COLUMNS)
        .first(conn)
        .optional()?;
    Ok(entries(conn, row.into_iter().collect())?.pop())
}

fn entries(
    conn: &mut SqliteConnection,
    rows: Vec<EntryRow>,
) -> QueryResult<Vec<ShoppingListHistoryEntry>> {
    let ids: Vec<Uuid> = rows.iter().map(|(id, ..)| *id).collect();
    let mut events = load_events(conn, &ids)?;
    Ok(rows
        .into_iter()
        .map(
            |(id, created_at, undoes_id, undone_by_id)| ShoppingListHistoryEntry {
                id,
                created_at,
                summary: summarize(&events.remove(&id).unwrap_or_default()),
                undoes_id,
                undone_by_id,
            },
        )
        .collect())
}

/// The events of each of the entries `ids`, in order.
pub fn load_events(
    conn: &mut SqliteConnection,
    ids: &[Uuid],
) -> QueryResult<HashMap<Uuid, Vec<Event>>> {
    let rows: Vec<EventRow> = shopping_list_history_events::table
        .filter(shopping_list_history_events::history_id.eq_any(ids))
        .select((
            shopping_list_history_events::history_id,
            shopping_list_history_events::item_id,
            shopping_list_history_events::op,
            shopping_list_history_events::before,
            shopping_list_history_events::after,
        ))
        .order((
            shopping_list_history_events::history_id.asc(),
            shopping_list_history_events::position.asc(),
        ))
        .load(conn)?;
    let mut events: HashMap<Uuid, Vec<Event>> = HashMap::new();
    for (history_id, item_id, op, before, after) in rows {
        events.entry(history_id).or_default().push(Event {
            item_id,
            op: parse_op(&op)?,
            before: before.map(from_json).transpose()?,
            after: after.map(from_json).transpose()?,
        });
    }
    Ok(events)
}

/// Store `events` as a new entry, marking `undoes` as undone by it, and
/// return its id. Changing nothing stores nothing.
pub fn record(
    conn: &mut SqliteConnection,
    user_id: Uuid,
    now: DateTime<Utc>,
    events: &[Event],
    undoes: Option<Uuid>,
) -> QueryResult<Option<Uuid>> {
    if events.is_empty() {
        return Ok(None);
    }
    let id = Uuid::now_v7();
    diesel::insert_into(shopping_list_history::table)
        .values((
            shopping_list_history::id.eq(id),
            shopping_list_history::user_id.eq(user_id),
            shopping_list_history::created_at.eq(now),
            shopping_list_history::undoes_id.eq(undoes),
        ))
        .execute(conn)?;
    for (position, event) in events.iter().enumerate() {
        diesel::insert_into(shopping_list_history_events::table)
            .values((
                shopping_list_history_events::history_id.eq(id),
                shopping_list_history_events::position.eq(position as i32),
                shopping_list_history_events::item_id.eq(event.item_id),
                shopping_list_history_events::op.eq(op_name(event.op)),
                shopping_list_history_events::before.eq(event.before.as_ref().map(to_json)),
                shopping_list_history_events::after.eq(event.after.as_ref().map(to_json)),
            ))
            .execute(conn)?;
    }
    if let Some(undoes) = undoes {
        diesel::update(shopping_list_history::table.find(undoes))
            .set(shopping_list_history::undone_by_id.eq(id))
            .execute(conn)?;
    }
    Ok(Some(id))
}

/// The item `id`, deleted or not, if it exists.
pub fn load_item(conn: &mut SqliteConnection, id: Uuid) -> QueryResult<Option<ItemRow>> {
    let row: Option<ItemColumns> = shopping_list_items::table
        .find(id)
        .select((
            shopping_list_items::user_id,
            shopping_list_items::deleted_at.is_null(),
            shopping_list_items::item,
            shopping_list_items::amount,
            shopping_list_items::note,
            shopping_list_items::is_checked,
            shopping_list_items::sort_order,
            shopping_list_items::category_override,
        ))
        .first(conn)
        .optional()?;
    Ok(row.map(
        |(user_id, live, item, amount, note, is_checked, sort_order, category_override)| ItemRow {
            user_id,
            live,
            fields: ItemFields {
                item,
                amount,
                note,
                is_checked,
                sort_order,
                category_override,
            },
        },
    ))
}

fn op_name(op: OpKind) -> &'static str {
    match op {
        OpKind::Create => "create",
        OpKind::Update => "update",
        OpKind::Delete => "delete",
        OpKind::Restore => "restore",
    }
}

fn parse_op(name: &str) -> QueryResult<OpKind> {
    match name {
        "create" => Ok(OpKind::Create),
        "update" => Ok(OpKind::Update),
        "delete" => Ok(OpKind::Delete),
        "restore" => Ok(OpKind::Restore),
        _ => Err(diesel::result::Error::DeserializationError(
            format!("unknown history op {name:?}").into(),
        )),
    }
}

fn to_json(fields: &ItemFields) -> serde_json::Value {
    serde_json::to_value(fields).expect("item fields serialize")
}

fn from_json(value: serde_json::Value) -> QueryResult<ItemFields> {
    serde_json::from_value(value)
        .map_err(|e| diesel::result::Error::DeserializationError(Box::new(e)))
}

/// What an event did to its item, in the order a summary lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Change {
    Added,
    Restored,
    Checked,
    Unchecked,
    Edited,
    Moved,
    Deleted,
}

impl Change {
    fn of(event: &Event) -> Option<Self> {
        match (&event.before, &event.after) {
            (None, Some(_)) if event.op == OpKind::Restore => Some(Self::Restored),
            (None, Some(_)) => Some(Self::Added),
            (Some(_), None) => Some(Self::Deleted),
            (Some(before), Some(after)) => {
                // Whether `field` is the only field that changed.
                let only = |field: fn(&mut ItemFields, &ItemFields)| {
                    let mut changed = before.clone();
                    field(&mut changed, after);
                    &changed == after
                };
                Some(if only(|f, after| f.is_checked = after.is_checked) {
                    if after.is_checked {
                        Self::Checked
                    } else {
                        Self::Unchecked
                    }
                } else if only(|f, after| f.sort_order = after.sort_order) {
                    Self::Moved
                } else {
                    Self::Edited
                })
            }
            (None, None) => None,
        }
    }

    fn verb(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Restored => "restored",
            Self::Checked => "checked",
            Self::Unchecked => "unchecked",
            Self::Edited => "edited",
            Self::Moved => "moved",
            Self::Deleted => "deleted",
        }
    }
}

/// A sentence saying what `events` did, e.g. "Checked milk; deleted eggs
/// and bread". Each item is named once per kind of change, by its latest
/// name.
pub fn summarize(events: &[Event]) -> String {
    let mut changes: Vec<(Change, Vec<(Uuid, &str)>)> = Vec::new();
    for event in events {
        let Some(change) = Change::of(event) else {
            continue;
        };
        let Some(name) = event.after.as_ref().or(event.before.as_ref()) else {
            continue;
        };
        let names = match changes.iter_mut().find(|(c, _)| *c == change) {
            Some((_, names)) => names,
            None => {
                changes.push((change, Vec::new()));
                &mut changes.last_mut().expect("just pushed").1
            }
        };
        match names.iter_mut().find(|(id, _)| *id == event.item_id) {
            Some(named) => named.1 = &name.item,
            None => names.push((event.item_id, &name.item)),
        }
    }
    changes.sort_by_key(|(change, _)| *change);

    let sentence = changes
        .iter()
        .map(|(change, names)| {
            let names: Vec<&str> = names.iter().map(|(_, name)| *name).collect();
            format!("{} {}", change.verb(), list_names(&names))
        })
        .collect::<Vec<_>>()
        .join("; ");
    let mut chars = sentence.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => "No changes".to_string(),
    }
}

/// "a", "a and b", "a, b and c", or "a, b and 3 more".
fn list_names(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [only] => (*only).to_string(),
        [rest @ .., last] if names.len() <= 3 => format!("{} and {last}", rest.join(", ")),
        [first, second, rest @ ..] => format!("{first}, {second} and {} more", rest.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(item: &str) -> ItemFields {
        ItemFields {
            item: item.to_string(),
            amount: None,
            note: None,
            is_checked: false,
            sort_order: 1,
            category_override: None,
        }
    }

    fn event(op: OpKind, before: Option<ItemFields>, after: Option<ItemFields>) -> Event {
        Event {
            item_id: Uuid::now_v7(),
            op,
            before,
            after,
        }
    }

    fn checked(item: &str) -> Event {
        let after = ItemFields {
            is_checked: true,
            ..fields(item)
        };
        event(OpKind::Update, Some(fields(item)), Some(after))
    }

    fn deleted(item: &str) -> Event {
        event(OpKind::Delete, Some(fields(item)), None)
    }

    #[test]
    fn names_each_kind_of_change_in_a_fixed_order() {
        let events = vec![
            deleted("eggs"),
            checked("milk"),
            event(OpKind::Create, None, Some(fields("flour"))),
            deleted("bread"),
        ];
        assert_eq!(
            summarize(&events),
            "Added flour; checked milk; deleted eggs and bread"
        );
    }

    #[test]
    fn tells_restores_unchecks_moves_and_edits_apart() {
        let unchecked = ItemFields {
            is_checked: true,
            ..fields("milk")
        };
        let moved = ItemFields {
            sort_order: 5,
            ..fields("eggs")
        };
        let renamed = ItemFields {
            amount: Some("2".to_string()),
            ..fields("butter")
        };
        let events = vec![
            event(OpKind::Restore, None, Some(fields("salt"))),
            event(OpKind::Update, Some(unchecked), Some(fields("milk"))),
            event(OpKind::Update, Some(fields("eggs")), Some(moved)),
            event(OpKind::Update, Some(fields("butter")), Some(renamed)),
        ];
        assert_eq!(
            summarize(&events),
            "Restored salt; unchecked milk; edited butter; moved eggs"
        );
    }

    #[test]
    fn a_long_list_names_two_and_counts_the_rest() {
        let events: Vec<Event> = ["a", "b", "c", "d", "e"].map(deleted).into();
        assert_eq!(summarize(&events), "Deleted a, b and 3 more");
        assert_eq!(summarize(&events[..3]), "Deleted a, b and c");
    }

    #[test]
    fn an_item_changed_twice_is_named_once_by_its_latest_name() {
        let first = event(
            OpKind::Update,
            Some(fields("milk")),
            Some(fields("oat milk")),
        );
        let second = Event {
            item_id: first.item_id,
            ..event(
                OpKind::Update,
                Some(fields("oat milk")),
                Some(fields("soy milk")),
            )
        };
        assert_eq!(summarize(&[first, second]), "Edited soy milk");
    }
}
