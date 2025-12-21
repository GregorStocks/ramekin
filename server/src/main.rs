mod api;
mod auth;
mod db;
mod models;
mod schema;

use axum::extract::MatchedPath;
use axum::http::Request;
use axum::middleware;
use axum::Router;
use opentelemetry::trace::TracerProvider;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use std::env;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;
use tower_http::trace::TraceLayer;
use tracing::Span;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use utoipa_swagger_ui::SwaggerUi;

/// Application state shared across all handlers
pub type AppState = Arc<db::DbPool>;

/// Initialize telemetry with optional OpenTelemetry export.
/// If OTEL_EXPORTER_OTLP_ENDPOINT is set and reachable, traces are sent to the collector.
/// Otherwise, only console logging is used.
fn init_telemetry() {
    let fmt_layer = tracing_subscriber::fmt::layer();
    let env_filter = tracing_subscriber::EnvFilter::from_default_env();

    // Check if OTLP endpoint is configured
    let otel_endpoint = env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();

    if let Some(endpoint) = otel_endpoint {
        // Parse the endpoint to check if it's reachable
        let host_port = endpoint
            .trim_start_matches("http://")
            .trim_start_matches("https://");

        // Quick TCP check to see if the collector is up (resolve hostname first)
        let is_reachable = host_port
            .to_socket_addrs()
            .ok()
            .and_then(|mut addrs| addrs.next())
            .map(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_ok())
            .unwrap_or(false);

        if is_reachable {
            let service_name =
                env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "ramekin-server".to_string());

            let resource = opentelemetry_sdk::Resource::builder()
                .with_service_name(service_name.clone())
                .build();

            // Set up trace exporter
            let trace_exporter = opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_endpoint(&endpoint)
                .build()
                .expect("Failed to create OTLP trace exporter");

            let trace_provider = SdkTracerProvider::builder()
                .with_batch_exporter(trace_exporter)
                .with_resource(resource.clone())
                .build();

            let tracer = trace_provider.tracer("ramekin-server");
            opentelemetry::global::set_tracer_provider(trace_provider);

            let otel_trace_layer = tracing_opentelemetry::layer().with_tracer(tracer);

            // Set up log exporter
            let log_exporter = opentelemetry_otlp::LogExporter::builder()
                .with_tonic()
                .with_endpoint(&endpoint)
                .build()
                .expect("Failed to create OTLP log exporter");

            let log_provider = SdkLoggerProvider::builder()
                .with_batch_exporter(log_exporter)
                .with_resource(resource)
                .build();

            let otel_log_layer = OpenTelemetryTracingBridge::new(&log_provider);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer)
                .with(otel_trace_layer)
                .with(otel_log_layer)
                .init();

            tracing::info!(
                "OpenTelemetry enabled, exporting traces and logs to {} as {}",
                endpoint,
                service_name
            );
        } else {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer)
                .init();

            tracing::info!(
                "OpenTelemetry endpoint {} not reachable, using console logging only",
                endpoint
            );
        }
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();

        tracing::debug!("OTEL_EXPORTER_OTLP_ENDPOINT not set, using console logging only");
    }
}

#[tokio::main]
async fn main() {
    // Check for --openapi flag to dump spec and exit
    if env::args().any(|arg| arg == "--openapi") {
        let spec = api::openapi().to_pretty_json().unwrap();
        println!("{}", spec);
        return;
    }

    init_telemetry();

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

                    // Don't create a span at all for noisy endpoints
                    if matched_path == "/api/test/unauthed-ping" {
                        tracing::trace_span!("http_request")
                    } else {
                        tracing::info_span!(
                            "http_request",
                            method = %request.method(),
                            path = %matched_path,
                        )
                    }
                })
                .on_request(|_request: &Request<_>, _span: &Span| {})
                .on_response(
                    |response: &axum::http::Response<_>,
                     latency: std::time::Duration,
                     span: &Span| {
                        // Skip logging for noisy endpoints (trace-level spans)
                        if span.metadata().map(|m| m.level()) == Some(&tracing::Level::TRACE) {
                            return;
                        }
                        let status = response.status().as_u16();
                        if status >= 500 {
                            tracing::error!(
                                status = %status,
                                latency_ms = %latency.as_millis(),
                                "request failed with server error"
                            );
                        } else {
                            tracing::info!(
                                status = %status,
                                latency_ms = %latency.as_millis(),
                                "request completed"
                            );
                        }
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
