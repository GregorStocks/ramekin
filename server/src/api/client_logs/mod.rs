pub mod create;
pub mod get;
pub mod list;

use crate::AppState;
use axum::extract::DefaultBodyLimit;
use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

/// Body limit above MAX_CONTENT_BYTES so the in-handler check produces the
/// precise 413 message for oversized `content`; this layer only backstops
/// pathological bodies.
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/",
            get(list::list_client_logs)
                .post(create::create_client_log)
                .layer(DefaultBodyLimit::max(MAX_BODY_BYTES)),
        )
        .route("/{id}", get(get::get_client_log))
}

#[derive(OpenApi)]
#[openapi(
    paths(create::create_client_log, list::list_client_logs, get::get_client_log),
    components(schemas(
        create::CreateClientLogRequest,
        create::CreateClientLogResponse,
        list::ClientLogSummary,
        list::ListClientLogsResponse,
        get::GetClientLogResponse
    ))
)]
pub struct ApiDoc;
