mod api;
mod auth;
mod db;
mod models;
mod schema;

use api::{
    paths, ErrorResponse, LoginRequest, LoginResponse, PingResponse, SignupRequest, SignupResponse,
};
use axum::middleware;
use axum::routing::{get, post};
use axum::{Json, Router};
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(unauthed_ping, auth::handlers::signup, auth::handlers::login, auth::handlers::ping),
    components(schemas(
        PingResponse,
        SignupRequest,
        SignupResponse,
        LoginRequest,
        LoginResponse,
        ErrorResponse
    )),
    modifiers(&SecurityAddon)
)]
struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::Http::new(
                        utoipa::openapi::security::HttpAuthScheme::Bearer,
                    ),
                ),
            );
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = Arc::new(db::create_pool(&database_url));

    // Public routes - no authentication required
    // Add new public routes here (health checks, login, signup, etc.)
    let (public_router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .route(paths::UNAUTHED_PING, get(unauthed_ping))
        .route(paths::SIGNUP, post(auth::signup))
        .route(paths::LOGIN, post(auth::login))
        .split_for_parts();

    // Protected routes - authentication required by default
    // ADD NEW ROUTES HERE - they will automatically require auth
    let protected_router =
        Router::new()
            .route(paths::PING, get(auth::ping))
            .layer(middleware::from_fn_with_state(
                pool.clone(),
                auth::require_auth,
            ));

    let swagger_ui = SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api);

    let app = public_router
        .merge(protected_router)
        .merge(swagger_ui)
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    tracing::info!("Server listening on {}", listener.local_addr().unwrap());
    tracing::info!("Swagger UI available at http://localhost:3000/swagger-ui/");
    tracing::info!("OpenAPI spec available at http://localhost:3000/api-docs/openapi.json");
    tracing::info!("Hot reload is enabled!");

    axum::serve(listener, app).await.unwrap();
}

#[utoipa::path(
    get,
    path = "/api/test/unauthed-ping",
    responses(
        (status = 200, description = "Unauthed ping response", body = PingResponse)
    )
)]
async fn unauthed_ping() -> Json<PingResponse> {
    Json(PingResponse {
        message: "unauthed-ping".to_string(),
    })
}
