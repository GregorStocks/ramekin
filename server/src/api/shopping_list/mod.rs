pub mod history;
pub mod undo;

use crate::AppState;
use axum::routing::{get, post};
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for /api/shopping-list endpoints (mounted at
/// /api/shopping-list). Item writes go through `POST /api/sync/shopping-list`.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/history", get(history::list_shopping_list_history))
        .route("/history/{id}/undo", post(undo::undo_shopping_list_change))
}

#[derive(OpenApi)]
#[openapi(
    paths(history::list_shopping_list_history, undo::undo_shopping_list_change),
    components(schemas(
        history::ShoppingListHistoryResponse,
        history::ShoppingListHistoryEntry,
    ))
)]
pub struct ApiDoc;
