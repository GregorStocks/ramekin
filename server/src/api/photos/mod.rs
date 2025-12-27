pub mod get;
pub mod get_thumbnail;
pub mod upload;

use crate::AppState;
use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for /api/photos endpoints (mounted at /api/photos)
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", axum::routing::post(upload::upload))
        .route("/{id}", get(get::get_photo))
        .route("/{id}/thumbnail", get(get_thumbnail::get_photo_thumbnail))
}

#[derive(OpenApi)]
#[openapi(
    paths(upload::upload, get::get_photo, get_thumbnail::get_photo_thumbnail,),
    components(schemas(upload::UploadPhotoRequest, upload::UploadPhotoResponse,))
)]
pub struct ApiDoc;
