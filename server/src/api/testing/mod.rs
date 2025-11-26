pub mod ping;

use crate::AppState;
use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for /api/test endpoints (mounted at /api/test)
pub fn router() -> Router<AppState> {
    Router::new().route("/ping", get(ping::ping))
}

#[derive(OpenApi)]
#[openapi(paths(ping::ping), components(schemas(ping::PingResponse)))]
pub struct ApiDoc;
