pub mod create;
pub mod delete;
pub mod list;
pub mod rename;

use crate::AppState;
use axum::routing::{delete, get};
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for /api/tags endpoints (mounted at /api/tags)
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list::list_all_tags).post(create::create_tag))
        .route(
            "/{id}",
            delete(delete::delete_tag).patch(rename::rename_tag),
        )
}

#[derive(OpenApi)]
#[openapi(
    paths(
        list::list_all_tags,
        create::create_tag,
        delete::delete_tag,
        rename::rename_tag
    ),
    components(schemas(
        list::TagsListResponse,
        list::TagItem,
        create::CreateTagRequest,
        create::CreateTagResponse,
        rename::RenameTagRequest,
        rename::RenameTagResponse,
    ))
)]
pub struct ApiDoc;
