pub mod me;

use crate::AppState;
use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for /api/users endpoints (mounted at /api/users)
pub fn router() -> Router<AppState> {
    Router::new().route("/me", get(me::me))
}

#[derive(OpenApi)]
#[openapi(paths(me::me), components(schemas(me::MeResponse)))]
pub struct ApiDoc;
