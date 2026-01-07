use diesel::connection::{set_default_instrumentation, Instrumentation, InstrumentationEvent};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::cell::RefCell;
use tracing::Span;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../migrations");

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;
pub type DbConn = r2d2::PooledConnection<ConnectionManager<PgConnection>>;

/// Helper macro to get a database connection from a pool.
/// Returns early with a 500 error response if the connection fails.
///
/// Usage:
/// ```ignore
/// let mut conn = get_conn!(pool);
/// ```
#[macro_export]
macro_rules! get_conn {
    ($pool:expr) => {
        match $pool.get() {
            Ok(c) => c,
            Err(_) => {
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json($crate::api::ErrorResponse {
                        error: "Database connection failed".to_string(),
                    }),
                )
                    .into_response()
            }
        }
    };
}

// Thread-local storage for tracking active database spans.
// This allows us to properly close spans when queries complete.
thread_local! {
    static ACTIVE_SPAN: RefCell<Option<(Span, tracing::span::EnteredSpan)>> = const { RefCell::new(None) };
}

/// Tracing-based instrumentation for Diesel database operations.
struct TracingInstrumentation;

impl Instrumentation for TracingInstrumentation {
    fn on_connection_event(&mut self, event: InstrumentationEvent<'_>) {
        match event {
            InstrumentationEvent::StartQuery { query, .. } => {
                let sql = format!("{}", query);
                let span = tracing::info_span!(
                    "db.query",
                    db.system = "postgresql",
                    db.statement = %sql,
                );
                tracing::debug!(parent: &span, "executing query");
                let entered = span.clone().entered();
                ACTIVE_SPAN.with(|cell| {
                    *cell.borrow_mut() = Some((span, entered));
                });
            }
            InstrumentationEvent::FinishQuery { error, .. } => {
                ACTIVE_SPAN.with(|cell| {
                    if let Some((span, _entered)) = cell.borrow_mut().take() {
                        if let Some(err) = error {
                            span.record("error", tracing::field::display(err));
                            tracing::warn!(parent: &span, error = %err, "query failed");
                        }
                        // _entered is dropped here, exiting the span
                    }
                });
            }
            InstrumentationEvent::BeginTransaction { depth, .. } => {
                tracing::debug!(depth = %depth, "begin transaction");
            }
            InstrumentationEvent::CommitTransaction { depth, .. } => {
                tracing::debug!(depth = %depth, "commit transaction");
            }
            InstrumentationEvent::RollbackTransaction { depth, .. } => {
                tracing::warn!(depth = %depth, "rollback transaction");
            }
            _ => {}
        }
    }
}

fn tracing_instrumentation() -> Option<Box<dyn Instrumentation>> {
    Some(Box::new(TracingInstrumentation))
}

pub fn create_pool(database_url: &str) -> DbPool {
    // Set up automatic tracing for all database operations
    set_default_instrumentation(tracing_instrumentation)
        .expect("Failed to set default instrumentation");

    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create database pool");

    // Run pending migrations on startup
    let mut conn = pool
        .get()
        .expect("Failed to get DB connection for migrations");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run database migrations");

    pool
}
