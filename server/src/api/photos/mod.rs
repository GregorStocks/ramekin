pub mod get;
pub mod get_thumbnail;
pub mod list;
pub mod upload;

use crate::AppState;
use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for /api/photos endpoints (mounted at /api/photos)
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list::list_photos).post(upload::upload))
        .route("/{id}", get(get::get_photo))
        .route("/{id}/thumbnail", get(get_thumbnail::get_photo_thumbnail))
}

#[derive(OpenApi)]
#[openapi(
    paths(
        upload::upload,
        get::get_photo,
        get_thumbnail::get_photo_thumbnail,
        list::list_photos,
    ),
    components(schemas(
        upload::UploadPhotoRequest,
        upload::UploadPhotoResponse,
        list::ListPhotosResponse,
        list::PhotoSummary,
    ))
)]
pub struct ApiDoc;
