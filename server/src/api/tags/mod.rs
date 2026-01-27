pub mod create;
pub mod delete;
pub mod list;

use crate::AppState;
use axum::routing::{delete, get};
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for /api/tags endpoints (mounted at /api/tags)
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list::list_all_tags).post(create::create_tag))
        .route("/{id}", delete(delete::delete_tag))
}

#[derive(OpenApi)]
#[openapi(
    paths(list::list_all_tags, create::create_tag, delete::delete_tag,),
    components(schemas(
        list::TagsListResponse,
        list::TagItem,
        create::CreateTagRequest,
        create::CreateTagResponse,
    ))
)]
pub struct ApiDoc;
