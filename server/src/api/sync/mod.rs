//! The sync feeds. Every synced domain shares one protocol
//! (doc/sync-design.md): a cursor from the sync clock, a response of changed
//! rows plus tombstones, a flagged full resync for a cursor this database
//! can't answer, and, for writable domains, field ops applied
//! last-writer-wins in the same request.

pub mod meal_plans;
pub mod recipes;
pub mod shopping_list;
pub mod tags;

use crate::db;
use crate::AppState;
use axum::routing::{get, post};
use axum::Router;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

/// Returns the router for /api/sync endpoints (mounted at /api/sync)
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/recipes", get(recipes::sync_recipes))
        .route("/tags", get(tags::sync_tags))
        .route("/shopping-list", post(shopping_list::sync_shopping_list))
        .route("/meal-plans", post(meal_plans::sync_meal_plans))
}

#[derive(OpenApi)]
#[openapi(
    paths(
        recipes::sync_recipes,
        tags::sync_tags,
        shopping_list::sync_shopping_list,
        meal_plans::sync_meal_plans,
    ),
    components(schemas(OpKind, OpResult))
)]
pub struct ApiDoc;

/// What a write op does to its row.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpKind {
    /// Create the row under the op's id. Replaying it is a no-op.
    Create,
    /// Set the fields the op names; fields it leaves out keep their value.
    Update,
    /// Soft-delete the row.
    Delete,
}

/// The outcome of one write op, reported in request order.
#[derive(Debug, Clone, Copy, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpResult {
    /// The op took effect, or already had (a replayed create, a delete of a
    /// deleted row).
    Applied,
    /// The row is deleted or doesn't exist; drop it.
    Gone,
    /// A meal plan op would plan the same recipe twice for one meal. The
    /// server's copy of an updated plan comes back in `changes`.
    Duplicate,
    /// A meal plan create names a recipe that is missing or deleted.
    RecipeMissing,
}

/// The client's cursor resolved against this database. Read it inside the
/// sync's transaction, before the feed's reads (and after a write
/// endpoint's own writes), so they share one snapshot.
#[derive(Debug, Clone, Copy)]
pub struct Window {
    /// The cursor to hand back.
    pub cursor: i64,
    /// Where the delta starts, or None for a full resync: the client sent no
    /// cursor, or one this database can't answer (see `db::SyncCursor::since`).
    pub since: Option<i64>,
}

impl Window {
    pub fn read(conn: &mut SqliteConnection, client_cursor: Option<i64>) -> QueryResult<Self> {
        let sync_cursor = db::sync_cursor(conn)?;
        Ok(Self {
            cursor: sync_cursor.value,
            since: sync_cursor.since(client_cursor),
        })
    }

    pub fn full_resync(&self) -> bool {
        self.since.is_none()
    }
}

/// A nullable text field on a create: "" means no value.
pub fn text_or_none(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

/// A nullable text field on an update: absent keeps `current`, "" clears it.
/// (Clients' generated encoders drop nil fields, so null can't mean clear.)
pub fn patch_text(update: Option<String>, current: Option<String>) -> Option<String> {
    match update {
        None => current,
        Some(value) => text_or_none(Some(value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_text_keeps_absent_clears_blank_and_sets_value() {
        let current = Some("old".to_string());
        assert_eq!(patch_text(None, current.clone()), current);
        assert_eq!(patch_text(Some(String::new()), current.clone()), None);
        assert_eq!(
            patch_text(Some("new".to_string()), current),
            Some("new".to_string())
        );
    }
}
