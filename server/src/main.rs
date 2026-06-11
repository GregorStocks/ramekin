#![deny(clippy::print_stdout, clippy::print_stderr)]

mod api;
mod auth;
mod db;
mod models;
mod photos;
mod raw_sql;
mod schema;
mod scraping;
mod tags;
mod telemetry;
mod types;

use axum::extract::MatchedPath;
use axum::http::Request;
use axum::middleware;
use axum::routing::post;
use axum::Router;
use listenfd::ListenFd;
use opentelemetry::trace::TracerProvider;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use std::env;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::Span;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use utoipa_swagger_ui::SwaggerUi;

/// Application state shared across all handlers
pub type AppState = Arc<db::DbPool>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ListenerSource {
    DirectBind,
    SocketActivation,
}

/// Initialize telemetry with optional OpenTelemetry export.
/// If OTEL_EXPORTER_OTLP_ENDPOINT is set and reachable, traces are sent to the collector.
/// Otherwise, only console logging is used.
fn init_telemetry() {
    let fmt_layer = tracing_subscriber::fmt::layer().with_ansi(env::var("NO_COLOR").is_err());
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
                .with(telemetry::DbQueryCountingLayer)
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
                .with(telemetry::DbQueryCountingLayer)
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
            .with(telemetry::DbQueryCountingLayer)
            .init();

        tracing::debug!("OTEL_EXPORTER_OTLP_ENDPOINT not set, using console logging only");
    }
}

async fn bind_listener(port: u16) -> (tokio::net::TcpListener, ListenerSource) {
    let mut listenfd = ListenFd::from_env();
    if let Some(listener) = listenfd
        .take_tcp_listener(0)
        .expect("failed to read externally managed listener")
    {
        listener
            .set_nonblocking(true)
            .expect("failed to make externally managed listener nonblocking");
        let listener = tokio::net::TcpListener::from_std(listener)
            .expect("failed to convert externally managed listener");
        return (listener, ListenerSource::SocketActivation);
    }

    let bind_addr = format!("0.0.0.0:{}", port);
    tracing::debug!("Attempting to bind to {}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {}: {}", bind_addr, e));
    (listener, ListenerSource::DirectBind)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl-c signal");
    };

    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to listen for terminate signal");

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await;
    }

    tracing::info!("Shutdown signal received, draining existing connections");
}

#[tokio::main]
async fn main() {
    // Check for --openapi flag to dump spec and exit
    if env::args().any(|arg| arg == "--openapi") {
        let spec = api::openapi().to_pretty_json().unwrap();
        #[allow(clippy::print_stdout)]
        {
            println!("{}", spec);
        }
        return;
    }

    init_telemetry();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool: AppState = Arc::new(db::create_pool(&database_url));

    // Spawn background backfill for photo dimensions (populates width/height
    // for photos created before the column existed).
    photos::spawn_dimension_backfill(pool.clone());

    // Public routes (no auth required)
    let public_router = api::public::router();

    // CORS layer for bookmarklet requests (scrape endpoints need cross-origin access)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Protected routes (auth required)
    // Note: layer order is reversed (last added = first executed)
    // So auth runs first, then CORS adds headers to response
    let protected_router = Router::new()
        .nest("/api/test", api::testing::router())
        .nest("/api/photos", api::photos::router())
        .nest("/api/recipes", api::recipes::router())
        .nest("/api/scrape", api::scrape::router())
        .nest("/api/tags", api::tags::router())
        .nest("/api/meal-plans", api::meal_plans::router())
        .nest("/api/shopping-list", api::shopping_list::router())
        .route("/api/enrich", post(api::enrich::enrich_recipe))
        .route(
            "/api/enrich/custom",
            post(api::enrich::custom_enrich_recipe),
        )
        .route("/api/import/recipe", post(api::import::import_recipe))
        .route("/api/import/photos", post(api::import::import_from_photos))
        .layer(middleware::from_fn_with_state(
            pool.clone(),
            auth::require_auth,
        ))
        .layer(cors);

    let swagger_ui = SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api::openapi());

    let app = Router::new()
        .merge(public_router)
        .merge(protected_router)
        .merge(swagger_ui)
        .with_state(pool)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    let method = request.method().as_str();
                    let route = request
                        .extensions()
                        .get::<MatchedPath>()
                        .map(MatchedPath::as_str)
                        .unwrap_or(request.uri().path());

                    // Use "METHOD /route" as span name per OTel semantic conventions
                    let span_name = format!("{} {}", method, route);

                    // Don't create a span at all for noisy endpoints
                    if route == "/api/test/unauthed-ping" {
                        tracing::trace_span!("http_request")
                    } else {
                        tracing::info_span!(
                            "http_request",
                            otel.name = %span_name,
                            http.request.method = %method,
                            http.route = %route,
                            db.query_count = tracing::field::Empty,
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
                        let query_count = telemetry::get_query_count().unwrap_or(0);

                        // Record query count on span for OpenTelemetry export
                        span.record("db.query_count", query_count);

                        if status >= 500 {
                            tracing::error!(
                                status = %status,
                                latency_ms = %latency.as_millis(),
                                db.query_count = query_count,
                                "request failed with server error"
                            );
                        } else {
                            tracing::info!(
                                status = %status,
                                latency_ms = %latency.as_millis(),
                                db.query_count = query_count,
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
        )
        .layer(middleware::from_fn(
            telemetry::db_query_count_header_middleware,
        ))
        .layer(middleware::from_fn(telemetry::query_counting_middleware))
        // Outermost: reshape any framework-level error (extractor rejections,
        // routing fallbacks) into the structured `{ code, error }` body.
        .layer(middleware::from_fn(api::error::ensure_coded_errors));

    let port: u16 = env::var("PORT")
        .expect("PORT environment variable required")
        .parse()
        .expect("PORT must be a valid port number");
    let (listener, listener_source) = bind_listener(port).await;
    let addr = listener.local_addr().unwrap();

    match listener_source {
        ListenerSource::DirectBind => tracing::info!("Server listening on {}", addr),
        ListenerSource::SocketActivation => {
            tracing::info!("Server listening on {} via socket activation", addr)
        }
    }
    tracing::info!(
        "Swagger UI available at http://localhost:{}/swagger-ui/",
        addr.port()
    );
    tracing::info!(
        "OpenAPI spec available at http://localhost:{}/api-docs/openapi.json",
        addr.port()
    );
    tracing::info!("Hot reload is enabled!");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::{bind_listener, ListenerSource};
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn bind_listener_falls_back_to_direct_bind_without_socket_activation() {
        let _guard = env_lock().lock().unwrap();
        unsafe {
            std::env::remove_var("LISTEN_FDS");
            std::env::remove_var("LISTEN_PID");
        }

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (listener, source) = runtime.block_on(bind_listener(0));
        let addr = listener.local_addr().unwrap();

        assert_eq!(source, ListenerSource::DirectBind);
        assert!(addr.port() > 0);
    }

    #[cfg(unix)]
    #[test]
    fn bind_listener_uses_socket_activation_when_listener_is_present() {
        use std::os::fd::IntoRawFd;

        unsafe extern "C" {
            fn close(fd: i32) -> i32;
            fn dup(fd: i32) -> i32;
            fn dup2(src: i32, dst: i32) -> i32;
        }

        struct FdRestore(i32);

        impl Drop for FdRestore {
            fn drop(&mut self) {
                unsafe {
                    if self.0 >= 0 {
                        assert!(dup2(self.0, 3) >= 0, "failed to restore fd 3");
                        assert_eq!(close(self.0), 0, "failed to close duplicated fd");
                    } else {
                        let _ = close(3);
                    }
                    std::env::remove_var("LISTEN_FDS");
                    std::env::remove_var("LISTEN_PID");
                }
            }
        }

        let _guard = env_lock().lock().unwrap();
        let restore = FdRestore(unsafe { dup(3) });
        unsafe {
            close(3);
        }
        let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let expected_addr = std_listener.local_addr().unwrap();
        let listener_fd = std_listener.into_raw_fd();

        unsafe {
            if listener_fd != 3 {
                assert!(dup2(listener_fd, 3) >= 0, "failed to set fd 3");
                assert_eq!(close(listener_fd), 0, "failed to close listener fd");
            }
            std::env::set_var("LISTEN_FDS", "1");
            std::env::remove_var("LISTEN_PID");
        }

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (listener, source) = runtime.block_on(bind_listener(1));

        assert_eq!(source, ListenerSource::SocketActivation);
        assert_eq!(listener.local_addr().unwrap(), expected_addr);

        drop(listener);
        drop(restore);
    }
}
