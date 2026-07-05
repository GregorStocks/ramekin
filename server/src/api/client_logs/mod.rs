pub mod create;

use crate::AppState;
use axum::extract::DefaultBodyLimit;
use axum::routing::post;
use axum::Router;
use utoipa::OpenApi;

/// Worst-case JSON escaping expands each content byte to a 6-byte \uXXXX
/// sequence; 64KB covers the JSON envelope. This must stay derived from
/// MAX_CONTENT_BYTES so the wire limit can never reject content the
/// in-handler check would accept.
const MAX_BODY_BYTES: usize = 6 * create::MAX_CONTENT_BYTES + 64 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create::create_client_log))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}

#[derive(OpenApi)]
#[openapi(
    paths(create::create_client_log),
    components(schemas(create::CreateClientLogRequest, create::CreateClientLogResponse))
)]
pub struct ApiDoc;
