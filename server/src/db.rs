use diesel::connection::{
    set_default_instrumentation, Instrumentation, InstrumentationEvent, SimpleConnection,
};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager, CustomizeConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::cell::{Cell, RefCell};
use std::sync::Mutex;
use tracing::Span;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../migrations");

pub type DbPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;
pub type DbConn = r2d2::PooledConnection<ConnectionManager<SqliteConnection>>;

/// The key a case-insensitive name (a username, a tag) is unique and looked
/// up by: `ramekin_core::search::fold_case`, which iOS mirrors for tags. The
/// database stores it in generated `username_key` / `name_key` columns
/// through the `fold_case` SQL function; compare lookups against this.
pub fn case_key(name: &str) -> String {
    ramekin_core::search::fold_case(name)
}

/// SQLite runs one write transaction at a time. Writers queue on this lock
/// instead of SQLite's busy handler, which retries on a timer and can starve
/// one writer behind a stream of others.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    /// Whether this thread holds `WRITE_LOCK`.
    static HOLDS_WRITE_LOCK: Cell<bool> = const { Cell::new(false) };
    /// Whether this thread is inside a write transaction, so a write nested
    /// in another becomes a savepoint.
    static IN_WRITE_TRANSACTION: Cell<bool> = const { Cell::new(false) };
}

/// Sets a thread-local flag until dropped, a panic included.
struct FlagGuard(&'static std::thread::LocalKey<Cell<bool>>);

impl FlagGuard {
    fn set(flag: &'static std::thread::LocalKey<Cell<bool>>) -> Self {
        flag.set(true);
        Self(flag)
    }
}

impl Drop for FlagGuard {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// The app-wide write lock, held until dropped. None when this thread
/// already holds it.
fn lock_writes() -> Option<(std::sync::MutexGuard<'static, ()>, FlagGuard)> {
    if HOLDS_WRITE_LOCK.get() {
        return None;
    }
    let lock = WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    Some((lock, FlagGuard::set(&HOLDS_WRITE_LOCK)))
}

/// Run `f` in a write transaction. It takes the app-wide write lock, then
/// `BEGIN IMMEDIATE`, so the transaction holds SQLite's write lock from its
/// first statement and never fails to upgrade from a read. Every write goes
/// through here, including single statements. Keep the closure short and
/// free of network calls: every other writer waits for it.
///
/// When the whole closure of a `run_blocking` is one write, use
/// [`run_blocking_write`] (or the API's `run_db_write`) instead.
pub fn write<T, E, F>(conn: &mut SqliteConnection, f: F) -> Result<T, E>
where
    F: FnOnce(&mut SqliteConnection) -> Result<T, E>,
    E: From<diesel::result::Error>,
{
    if IN_WRITE_TRANSACTION.get() {
        return conn.transaction(f);
    }
    let _lock = lock_writes();
    let _in_transaction = FlagGuard::set(&IN_WRITE_TRANSACTION);
    conn.immediate_transaction(f)
}

/// [`run_blocking`] with the whole closure in one [`write`]. Takes the write
/// lock before checking out a connection, so waiting for the lock doesn't tie
/// up a pooled connection that readers could use.
pub async fn run_blocking_write<T, E, F>(
    pool: &DbPool,
    f: F,
) -> Result<Result<T, E>, r2d2::PoolError>
where
    F: FnOnce(&mut SqliteConnection) -> Result<T, E> + Send + 'static,
    T: Send + 'static,
    E: From<diesel::result::Error> + Send + 'static,
{
    run_blocking_with(pool, true, move |conn| write(conn, f)).await
}

/// The sync cursor for a read starting now: one past the last change stamp
/// handed out. Every change committed after this read carries a stamp at or
/// above it, so the next sync's inclusive `>= cursor` filter picks it up.
/// SQLite runs one writer at a time and stamps in commit order, so nothing
/// below the cursor can still be in flight. Call it inside the sync's read
/// transaction, before the reads, so they share one snapshot.
pub fn sync_cursor(conn: &mut SqliteConnection) -> QueryResult<i64> {
    use crate::schema::sync_clock;
    let value: i64 = sync_clock::table.select(sync_clock::value).first(conn)?;
    Ok(value + 1)
}

/// `cursor` if it could have come from this database, else None. A cursor
/// above the current one was issued by another database (a restored backup,
/// a rebuilt file), so it can't say what the client has seen: answer with a
/// full sync instead of silently skipping every change below it.
pub fn valid_since(cursor: Option<i64>, current: i64) -> Option<i64> {
    cursor.filter(|&since| since <= current)
}

diesel::define_sql_function! {
    /// `ramekin_core::search::normalize_for_search`, registered on every
    /// connection. The search index triggers and the `source:` filter call
    /// it, so stored and queried text normalize identically.
    fn normalize_for_search(value: diesel::sql_types::Nullable<diesel::sql_types::Text>) -> diesel::sql_types::Nullable<diesel::sql_types::Text>;
}

diesel::define_sql_function! {
    /// [`case_key`], registered on every connection for the generated
    /// `username_key` and `name_key` columns.
    fn fold_case(value: diesel::sql_types::Text) -> diesel::sql_types::Text;
}

/// Per-connection setup: pragmas and the search normalization function.
#[derive(Debug)]
struct ConnectionSetup;

impl CustomizeConnection<SqliteConnection, r2d2::Error> for ConnectionSetup {
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), r2d2::Error> {
        setup_connection(conn).map_err(r2d2::Error::QueryError)
    }
}

/// Pragmas and functions every connection needs, pooled or not. Writes to
/// tables with generated keys or search triggers fail without the functions.
///
/// `synchronous = FULL` syncs the WAL on every commit, so a write the API
/// reported is durable across power loss. The busy timeout only backstops
/// [`write`], which already serializes writers; it comes first so the other
/// pragmas wait out a lock too.
pub fn setup_connection(conn: &mut SqliteConnection) -> QueryResult<()> {
    conn.batch_execute(
        "PRAGMA busy_timeout = 10000; PRAGMA synchronous = FULL; PRAGMA foreign_keys = ON;",
    )?;
    normalize_for_search_utils::register_impl(conn, |value: Option<String>| {
        value.map(|v| ramekin_core::search::normalize_for_search(&v))
    })?;
    fold_case_utils::register_impl(conn, |value: String| case_key(&value))
}

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
    run_blocking_with(pool, false, f).await
}

async fn run_blocking_with<T, F>(
    pool: &DbPool,
    write_lock: bool,
    f: F,
) -> Result<T, r2d2::PoolError>
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
            let _lock = if write_lock { lock_writes() } else { None };
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
                    db.system = "sqlite",
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

/// Open the database file (creating it and its directory if missing), set up
/// the connection, and run pending schema and data migrations. WAL mode is a
/// property of the file, so it's set here once rather than per connection
/// (switching it needs every other connection out of the way).
pub fn open_migrated(database_url: &str) -> SqliteConnection {
    if let Some(dir) = std::path::Path::new(database_url).parent() {
        std::fs::create_dir_all(dir)
            .unwrap_or_else(|e| panic!("Failed to create {}: {e}", dir.display()));
    }
    let mut conn = SqliteConnection::establish(database_url)
        .unwrap_or_else(|e| panic!("Failed to open database {database_url}: {e}"));
    setup_connection(&mut conn).expect("Failed to set up database connection");
    conn.batch_execute("PRAGMA journal_mode = WAL;")
        .expect("Failed to enable WAL mode");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run database migrations");
    crate::data_migrations::run_pending(&mut conn).expect("Failed to run data migrations");
    conn
}

pub fn create_pool(database_url: &str) -> DbPool {
    // Set up automatic tracing for all database operations
    set_default_instrumentation(tracing_instrumentation)
        .expect("Failed to set default instrumentation");

    // Migrate on one connection before the pool opens any.
    drop(open_migrated(database_url));

    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    r2d2::Pool::builder()
        // Connections are cheap file handles; plenty of them keeps writers
        // that wait on the write lock from starving readers of connections.
        .max_size(32)
        .min_idle(Some(4))
        .connection_customizer(Box::new(ConnectionSetup))
        .build(manager)
        .expect("Failed to create database pool")
}
