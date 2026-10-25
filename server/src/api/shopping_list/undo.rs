use super::history::{self, Event, ItemFields, ShoppingListHistoryEntry};
use crate::api::sync::shopping_list::{apply_ops, ShoppingListFields, ShoppingListOp};
use crate::api::sync::OpKind;
use crate::api::{run_db_write, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::ids::Uuid;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;

#[utoipa::path(
    post,
    path = "/api/shopping-list/history/{id}/undo",
    tag = "shopping_list",
    params(
        ("id" = Uuid, Path, description = "History entry ID")
    ),
    responses(
        (status = 200, description = "Undone; the entry recording the undo. Sync to see it.", body = ShoppingListHistoryEntry),
        (status = 404, description = "No such entry", body = ErrorResponse),
        (status = 409, description = "Already undone, or later changes replaced all of it", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn undo_shopping_list_change(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;

    let entry = run_db_write(&pool, move |conn| {
        let entry = history::load_entry(conn, user_id, id)?
            .ok_or_else(|| ApiError::not_found("History entry not found"))?;
        if entry.undone_by_id.is_some() {
            return Err(ApiError::conflict("Already undone"));
        }
        let events = history::load_events(conn, &[id])?
            .remove(&id)
            .unwrap_or_default();
        let mut ops = Vec::new();
        for (item_id, before, after) in net_changes(&events) {
            let current = history::load_item(conn, item_id)?
                .filter(|row| row.user_id == user_id)
                .map(|row| (row.live, row.fields));
            if let Some(op) = inverse(item_id, before, after, current) {
                ops.push(op);
            }
        }
        let undo_id = apply_ops(conn, user_id, ops, Some(id))?
            .history_id
            .ok_or_else(|| ApiError::conflict("Later changes already replaced all of it"))?;
        history::load_entry(conn, user_id, undo_id)?
            .ok_or_else(|| ApiError::internal("Undo entry missing"))
    })
    .await?;

    crate::ingredient_names::wake();

    Ok((StatusCode::OK, Json(entry)))
}

/// Each item's change across `events`, from its first state to its last,
/// latest-changed item first (the order to undo them in).
fn net_changes(events: &[Event]) -> Vec<(Uuid, Option<&ItemFields>, Option<&ItemFields>)> {
    let mut net: Vec<(Uuid, Option<&ItemFields>, Option<&ItemFields>)> = Vec::new();
    for event in events {
        match net.iter_mut().find(|(id, ..)| *id == event.item_id) {
            Some(change) => change.2 = event.after.as_ref(),
            None => net.push((event.item_id, event.before.as_ref(), event.after.as_ref())),
        }
    }
    net.reverse();
    net
}

/// The op that takes an item from `after` back to `before`, given its
/// `current` state (live, fields), or None when there's nothing to undo.
/// Fields changed again since are left alone: the later edit wins.
fn inverse(
    id: Uuid,
    before: Option<&ItemFields>,
    after: Option<&ItemFields>,
    current: Option<(bool, ItemFields)>,
) -> Option<ShoppingListOp> {
    let (live, current) = current?;
    let op = |op, fields| ShoppingListOp { op, id, fields };
    match (before, after) {
        (None, Some(_)) if live => Some(op(OpKind::Delete, ShoppingListFields::default())),
        // Every field, since the entry may have edited the item before
        // deleting it.
        (Some(before), None) if !live => Some(op(OpKind::Restore, all_fields(before))),
        (Some(before), Some(after)) if live => {
            // A field goes back only if it still holds what the entry set.
            fn back<T: PartialEq + Clone>(current: &T, before: &T, after: &T) -> Option<T> {
                (current == after && before != after).then(|| before.clone())
            }
            // "" clears a nullable field.
            let text = |current: &Option<String>, before: &Option<String>, after| {
                back(current, before, after).map(Option::unwrap_or_default)
            };
            let fields = ShoppingListFields {
                item: back(&current.item, &before.item, &after.item),
                amount: text(&current.amount, &before.amount, &after.amount),
                note: text(&current.note, &before.note, &after.note),
                is_checked: back(&current.is_checked, &before.is_checked, &after.is_checked),
                sort_order: back(&current.sort_order, &before.sort_order, &after.sort_order),
                category_override: text(
                    &current.category_override,
                    &before.category_override,
                    &after.category_override,
                ),
                ..ShoppingListFields::default()
            };
            let changes_something = fields.item.is_some()
                || fields.amount.is_some()
                || fields.note.is_some()
                || fields.is_checked.is_some()
                || fields.sort_order.is_some()
                || fields.category_override.is_some();
            changes_something.then(|| op(OpKind::Update, fields))
        }
        _ => None,
    }
}

/// Fields that set an item to `fields` ("" clears a nullable one).
fn all_fields(fields: &ItemFields) -> ShoppingListFields {
    ShoppingListFields {
        item: Some(fields.item.clone()),
        amount: Some(fields.amount.clone().unwrap_or_default()),
        note: Some(fields.note.clone().unwrap_or_default()),
        is_checked: Some(fields.is_checked),
        sort_order: Some(fields.sort_order),
        category_override: Some(fields.category_override.clone().unwrap_or_default()),
        ..ShoppingListFields::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(item: &str, is_checked: bool) -> ItemFields {
        ItemFields {
            item: item.to_string(),
            amount: None,
            note: None,
            is_checked,
            sort_order: 1,
            category_override: None,
        }
    }

    #[test]
    fn undoing_a_create_deletes_and_a_delete_restores() {
        let id = Uuid::now_v7();
        let milk = fields("milk", false);
        let op = inverse(id, None, Some(&milk), Some((true, milk.clone()))).unwrap();
        assert_eq!(op.op, OpKind::Delete);
        // Restored as the entry found it, though it was renamed before the
        // delete.
        let eggs = fields("eggs", false);
        let op = inverse(id, Some(&milk), None, Some((false, eggs))).unwrap();
        assert_eq!(op.op, OpKind::Restore);
        assert_eq!(op.fields.item.as_deref(), Some("milk"));
        // Already back where the entry started.
        assert!(inverse(id, Some(&milk), None, Some((true, milk.clone()))).is_none());
    }

    #[test]
    fn undoing_an_update_skips_fields_changed_since() {
        let id = Uuid::now_v7();
        let before = ItemFields {
            amount: Some("1".to_string()),
            ..fields("milk", false)
        };
        let after = ItemFields {
            amount: None,
            ..fields("oat milk", true)
        };
        // Renamed again since; still checked and without an amount.
        let current = ItemFields {
            item: "soy milk".to_string(),
            ..after.clone()
        };
        let op = inverse(id, Some(&before), Some(&after), Some((true, current))).unwrap();
        assert_eq!(op.op, OpKind::Update);
        assert_eq!(op.fields.item, None);
        assert_eq!(op.fields.is_checked, Some(false));
        assert_eq!(op.fields.amount.as_deref(), Some("1"));

        // Nothing left that the entry set.
        let current = ItemFields {
            amount: Some("2".to_string()),
            ..fields("soy milk", false)
        };
        assert!(inverse(id, Some(&before), Some(&after), Some((true, current))).is_none());
    }

    #[test]
    fn net_changes_span_each_items_events_latest_first() {
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        let milk = fields("milk", false);
        let checked = fields("milk", true);
        let eggs = fields("eggs", false);
        let events = vec![
            Event {
                item_id: a,
                op: OpKind::Update,
                before: Some(milk.clone()),
                after: Some(checked.clone()),
            },
            Event {
                item_id: b,
                op: OpKind::Create,
                before: None,
                after: Some(eggs.clone()),
            },
            Event {
                item_id: a,
                op: OpKind::Delete,
                before: Some(checked),
                after: None,
            },
        ];
        assert_eq!(
            net_changes(&events),
            vec![(b, None, Some(&eggs)), (a, Some(&milk), None)]
        );
    }
}
