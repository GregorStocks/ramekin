//! Telemetry utilities for tracking per-request metrics.
//!
//! This module provides a tracing Layer that counts database queries per HTTP request.

use axum::{body::Body, http::Request, middleware::Next, response::Response};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use tracing::{span::Id, Subscriber};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

tokio::task_local! {
    /// Task-local counter for database queries in the current request.
    /// This follows the async task across await points and thread migrations.
    static DB_QUERY_COUNTER: Arc<AtomicU32>;
}

/// Get the current database query count for this request, if available.
pub fn get_query_count() -> Option<u32> {
    DB_QUERY_COUNTER
        .try_with(|counter| counter.load(Ordering::Relaxed))
        .ok()
}

/// Capture the current request's query counter so a blocking task can keep
/// counting under it. Returns `None` outside a request scope.
pub fn current_query_counter() -> Option<Arc<AtomicU32>> {
    DB_QUERY_COUNTER.try_with(Arc::clone).ok()
}

/// Run `f` with `counter` installed as the task-local query counter, so
/// `db.query` spans created inside `f` are attributed to the originating
/// request. With `None`, `f` runs without a counter in scope.
pub fn with_query_counter<T>(counter: Option<Arc<AtomicU32>>, f: impl FnOnce() -> T) -> T {
    match counter {
        Some(counter) => DB_QUERY_COUNTER.sync_scope(counter, f),
        None => f(),
    }
}

/// A tracing Layer that counts db.query spans per HTTP request.
///
/// When a `db.query` span is created, the Layer increments the task-local counter.
/// This works because:
/// - The counter is initialized by `query_counting_middleware` for each request
/// - Diesel queries run on the blocking thread pool under [`with_query_counter`],
///   which re-enters the originating request's counter there
///   (see `db::run_blocking`)
pub struct DbQueryCountingLayer;

impl<S> Layer<S> for DbQueryCountingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, _attrs: &tracing::span::Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };

        // When a db.query span is created, increment the task-local counter
        if span.name() == "db.query" {
            let _ = DB_QUERY_COUNTER.try_with(|counter| {
                counter.fetch_add(1, Ordering::Relaxed);
            });
        }
    }
}

/// Middleware that initializes the per-request database query counter.
///
/// This must be added to the router AFTER the TraceLayer (so it runs BEFORE
/// the trace span is created, wrapping the entire request lifecycle).
pub async fn query_counting_middleware(request: Request<Body>, next: Next) -> Response {
    let counter = Arc::new(AtomicU32::new(0));
    DB_QUERY_COUNTER.scope(counter, next.run(request)).await
}

/// Middleware that adds X-DB-Query-Count header to responses.
/// Only enabled when TRACK_DB_QUERY_COUNT=1 environment variable is set.
pub async fn db_query_count_header_middleware(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;

    if std::env::var("TRACK_DB_QUERY_COUNT")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        if let Some(count) = get_query_count() {
            if let Ok(value) = axum::http::header::HeaderValue::from_str(&count.to_string()) {
                response.headers_mut().insert("X-DB-Query-Count", value);
            }
        }
    }

    response
}
