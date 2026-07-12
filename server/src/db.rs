use diesel::connection::{set_default_instrumentation, Instrumentation, InstrumentationEvent};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::cell::RefCell;
use tracing::Span;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../migrations");

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;
pub type DbConn = r2d2::PooledConnection<ConnectionManager<PgConnection>>;

/// Run blocking database work on tokio's blocking thread pool.
///
/// Diesel calls must never run directly on a runtime worker: a slow or
/// lock-blocked query parks the worker — and when that worker holds the IO
/// driver, the whole server stops answering until the query finishes. Every
/// async code path checks out its connection and runs its queries inside the
/// closure passed here instead.
///
/// A panic inside the closure propagates to the caller.
pub async fn run_blocking<T, F>(pool: &DbPool, f: F) -> Result<T, r2d2::PoolError>
where
    F: FnOnce(&mut DbConn) -> T + Send + 'static,
    T: Send + 'static,
{
    let pool = pool.clone();
    // Blocking threads carry no ambient request context, so re-enter the
    // caller's tracing span (keeps db.query spans parented to the request)
    // and its query counter (keeps X-DB-Query-Count accurate).
    let span = tracing::Span::current();
    let query_counter = crate::telemetry::current_query_counter();
    match tokio::task::spawn_blocking(move || {
        let _entered = span.entered();
        crate::telemetry::with_query_counter(query_counter, || {
            let mut conn = pool.get()?;
            Ok(f(&mut conn))
        })
    })
    .await
    {
        Ok(result) => result,
        Err(join_error) => match join_error.try_into_panic() {
            Ok(panic) => std::panic::resume_unwind(panic),
            Err(join_error) => panic!("database task cancelled: {join_error}"),
        },
    }
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
                // TODO: Diesel's DebugQuery format includes bindings after " -- binds:".
                // We strip them to avoid logging sensitive values. Replace this with a
                // cleaner API once Diesel provides one (e.g., a method to get just the SQL).
                let sql = format!("{}", query);
                let sql = sql.split(" -- binds:").next().unwrap_or(&sql);
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
