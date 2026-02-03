pub mod clear_checked;
pub mod create;
pub mod delete;
pub mod list;
pub mod sync;
pub mod update;

use crate::AppState;
use axum::routing::{delete as delete_method, get, post, put};
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for /api/shopping-list endpoints
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list::list_items).post(create::create_items))
        .route(
            "/{id}",
            put(update::update_item).delete(delete::delete_item),
        )
        .route("/sync", post(sync::sync_items))
        .route(
            "/clear-checked",
            delete_method(clear_checked::clear_checked),
        )
}

#[derive(OpenApi)]
#[openapi(
    paths(
        list::list_items,
        create::create_items,
        update::update_item,
        delete::delete_item,
        sync::sync_items,
        clear_checked::clear_checked
    ),
    components(schemas(
        list::ShoppingListResponse,
        list::ShoppingListItemResponse,
        create::CreateShoppingListRequest,
        create::CreateShoppingListItemRequest,
        create::CreateShoppingListResponse,
        update::UpdateShoppingListItemRequest,
        sync::SyncRequest,
        sync::SyncResponse,
        sync::SyncCreateItem,
        sync::SyncUpdateItem,
        sync::SyncCreatedItem,
        sync::SyncUpdatedItem,
        sync::SyncServerChange,
    ))
)]
pub struct ApiDoc;
