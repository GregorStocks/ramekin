pub mod create;
pub mod get;
pub mod list;

use crate::AppState;
use axum::extract::DefaultBodyLimit;
use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

/// Worst-case JSON escaping expands each content byte to a 6-byte \uXXXX
/// sequence; 64KB covers the JSON envelope. This must stay derived from
/// MAX_CONTENT_BYTES so the wire limit can never reject content the
/// in-handler check would accept.
const MAX_BODY_BYTES: usize = 6 * create::MAX_CONTENT_BYTES + 64 * 1024;

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
