pub mod bookmarklet_token;
pub mod me;

use crate::AppState;
use axum::routing::{get, post};
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for /api/users endpoints (mounted at /api/users)
pub fn router() -> Router<AppState> {
    Router::new().route("/me", get(me::me)).route(
        "/bookmarklet-token",
        post(bookmarklet_token::mint_bookmarklet_token),
    )
}

#[derive(OpenApi)]
#[openapi(
    paths(me::me, bookmarklet_token::mint_bookmarklet_token),
    components(schemas(me::MeResponse, bookmarklet_token::BookmarkletTokenResponse))
)]
pub struct ApiDoc;
