pub mod auth;
pub mod testing;

use crate::AppState;
use axum::routing::{get, post};
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for public endpoints (no auth required)
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/test/unauthed-ping",
            get(testing::unauthed_ping::unauthed_ping),
        )
        .route("/api/auth/signup", post(auth::signup::signup))
        .route("/api/auth/login", post(auth::login::login))
}

#[derive(OpenApi)]
#[openapi(
    paths(
        auth::login::login,
        auth::signup::signup,
        testing::unauthed_ping::unauthed_ping,
    ),
    components(schemas(
        auth::login::LoginRequest,
        auth::login::LoginResponse,
        auth::signup::SignupRequest,
        auth::signup::SignupResponse,
        testing::unauthed_ping::UnauthedPingResponse,
    ))
)]
pub struct ApiDoc;
