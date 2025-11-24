mod api;
mod auth;
mod db;
mod models;
mod schema;

use api::{
    paths, ErrorResponse, GarbagesResponse, HelloResponse, LoginRequest, LoginResponse,
    SignupRequest, SignupResponse,
};
use axum::{
    extract::State,
    routing::{get, post},
    Json,
};
use diesel::prelude::*;
use models::Garbage;
use schema::garbage;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(get_garbages, auth::signup, auth::login, auth::hello),
    components(schemas(
        GarbagesResponse,
        SignupRequest,
        SignupResponse,
        LoginRequest,
        LoginResponse,
        HelloResponse,
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

    let pool = db::create_pool(&database_url);

    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .route(paths::GARBAGES, get(get_garbages))
        .route(paths::SIGNUP, post(auth::signup))
        .route(paths::LOGIN, post(auth::login))
        .route(paths::HELLO, get(auth::hello))
        .split_for_parts();

    // Serve OpenAPI spec as JSON and Swagger UI
    let swagger_ui = SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api);

    let app = router.merge(swagger_ui).with_state(Arc::new(pool));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    tracing::info!("Server listening on {}", listener.local_addr().unwrap());
    tracing::info!("Swagger UI available at http://localhost:3000/swagger-ui/");
    tracing::info!("OpenAPI spec available at http://localhost:3000/api-docs/openapi.json");
    tracing::info!("Hot reload is enabled!");

    axum::serve(listener, app).await.unwrap();
}

#[utoipa::path(
    get,
    path = "/api/garbages",
    responses(
        (status = 200, description = "List of all garbages", body = GarbagesResponse)
    )
)]
async fn get_garbages(State(pool): State<Arc<db::DbPool>>) -> Json<GarbagesResponse> {
    let mut conn = pool.get().expect("Failed to get DB connection");

    let results = garbage::table
        .select(Garbage::as_select())
        .load(&mut conn)
        .expect("Error loading garbages");

    let garbages: Vec<String> = results.into_iter().map(|g| g.garbage_name).collect();

    Json(GarbagesResponse { garbages })
}
