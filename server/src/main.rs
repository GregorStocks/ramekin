mod api;
mod auth;
mod db;
mod models;
mod schema;

use axum::extract::MatchedPath;
use axum::http::Request;
use axum::middleware;
use axum::Router;
use std::env;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::Span;
use utoipa_swagger_ui::SwaggerUi;

/// Application state shared across all handlers
pub type AppState = Arc<db::DbPool>;

#[tokio::main]
async fn main() {
    // Check for --openapi flag to dump spec and exit
    if env::args().any(|arg| arg == "--openapi") {
        let spec = api::openapi().to_pretty_json().unwrap();
        println!("{}", spec);
        return;
    }

    tracing_subscriber::fmt::init();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool: AppState = Arc::new(db::create_pool(&database_url));

    // Public routes (no auth required)
    let public_router = api::public::router();

    // Protected routes (auth required)
    let protected_router = Router::new()
        .nest("/api/test", api::testing::router())
        .nest("/api/photos", api::photos::router())
        .nest("/api/recipes", api::recipes::router())
        .layer(middleware::from_fn_with_state(
            pool.clone(),
            auth::require_auth,
        ));

    let swagger_ui = SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api::openapi());

    let app = Router::new()
        .merge(public_router)
        .merge(protected_router)
        .merge(swagger_ui)
        .with_state(pool)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    let matched_path = request
                        .extensions()
                        .get::<MatchedPath>()
                        .map(MatchedPath::as_str)
                        .unwrap_or(request.uri().path());
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        path = %matched_path,
                    )
                })
                .on_request(|_request: &Request<_>, _span: &Span| {})
                .on_response(
                    |response: &axum::http::Response<_>,
                     latency: std::time::Duration,
                     _span: &Span| {
                        tracing::info!(
                            status = %response.status().as_u16(),
                            latency_ms = %latency.as_millis(),
                            "request completed"
                        );
                    },
                )
                .on_failure(
                    |error: tower_http::classify::ServerErrorsFailureClass,
                     latency: std::time::Duration,
                     _span: &Span| {
                        tracing::error!(
                            error = %error,
                            latency_ms = %latency.as_millis(),
                            "request failed"
                        );
                    },
                ),
        );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    tracing::info!("Server listening on {}", listener.local_addr().unwrap());
    tracing::info!("Swagger UI available at http://localhost:3000/swagger-ui/");
    tracing::info!("OpenAPI spec available at http://localhost:3000/api-docs/openapi.json");
    tracing::info!("Hot reload is enabled!");

    axum::serve(listener, app).await.unwrap();
}
