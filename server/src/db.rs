use diesel::connection::{
    set_default_instrumentation, Instrumentation, InstrumentationEvent, SimpleConnection,
};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager, CustomizeConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::cell::{Cell, RefCell};
use std::sync::Mutex;
use std::time::{Duration, Instant};
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

/// A write lock wait or hold longer than this logs a warning; shorter ones
/// log at debug. Every other writer queues behind a long hold, and SQLite
/// fails a writer whose wait on the file lock passes the busy timeout.
const SLOW_WRITE_LOCK: Duration = Duration::from_millis(500);

/// The app-wide write lock, held until dropped. On release it logs how long
/// it waited for the lock and how long it held it, after unlocking so a slow
/// log writer never holds up the next writer.
struct WriteLockGuard {
    lock: Option<std::sync::MutexGuard<'static, ()>>,
    _holds: FlagGuard,
    waited: Duration,
    acquired: Instant,
}

impl Drop for WriteLockGuard {
    fn drop(&mut self) {
        let held = self.acquired.elapsed();
        drop(self.lock.take());
        let waited_ms = self.waited.as_millis() as u64;
        let held_ms = held.as_millis() as u64;
        if self.waited >= SLOW_WRITE_LOCK || held >= SLOW_WRITE_LOCK {
            tracing::warn!(waited_ms, held_ms, "slow write lock");
        } else {
            tracing::debug!(waited_ms, held_ms, "released write lock");
        }
    }
}

/// The app-wide write lock, held until dropped. None when this thread
/// already holds it.
fn lock_writes() -> Option<WriteLockGuard> {
    if HOLDS_WRITE_LOCK.get() {
        return None;
    }
    let started = Instant::now();
    let lock = WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let acquired = Instant::now();
    Some(WriteLockGuard {
        lock: Some(lock),
        _holds: FlagGuard::set(&HOLDS_WRITE_LOCK),
        waited: acquired - started,
        acquired,
    })
}

/// Run `f` in a write transaction. It takes the app-wide write lock, then
/// `BEGIN IMMEDIATE`, so the transaction holds SQLite's write lock from its
/// first statement and never fails to upgrade from a read. Every write goes
/// through here, including single statements. Keep the closure short and
/// free of network calls: every other writer waits for it.
///
/// Run it under [`run_blocking_locked`] or [`run_blocking_write`] (the API's
/// `run_db_locked` / `run_db_write`), which take the lock before a pooled
/// connection.
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

/// [`run_blocking`] holding the write lock: takes the lock, then a
/// connection, so a writer waiting for the lock doesn't tie up a pooled
/// connection readers could use. For closures that read and then call
/// [`write`] themselves.
pub async fn run_blocking_locked<T, F>(pool: &DbPool, f: F) -> Result<T, r2d2::PoolError>
where
    F: FnOnce(&mut DbConn) -> T + Send + 'static,
    T: Send + 'static,
{
    run_blocking_with(pool, true, f).await
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
pub fn sync_cursor(conn: &mut SqliteConnection) -> QueryResult<SyncCursor> {
    use crate::schema::sync_clock;
    let (value, resync_floor): (i64, i64) = sync_clock::table
        .select((sync_clock::value, sync_clock::resync_floor))
        .first(conn)?;
    Ok(SyncCursor {
        value: value + 1,
        resync_floor,
    })
}

/// A sync's [`sync_cursor`], plus what it needs to vet the client's.
#[derive(Debug, Clone, Copy)]
pub struct SyncCursor {
    /// The cursor to hand back to the client.
    pub value: i64,
    /// The clock value of the last [`restamp_live`].
    resync_floor: i64,
}

impl SyncCursor {
    /// The client's `cursor` if this database can answer it with a delta,
    /// else None: a full resync, which the response flags so the client drops
    /// cached rows it omits. A cursor at or below the resync floor predates
    /// a restore or import that changed rows under the client without
    /// leaving tombstones. A cursor above the current one was issued by
    /// another database (a rebuilt file) and can't say what the client has
    /// seen.
    pub fn since(&self, cursor: Option<i64>) -> Option<i64> {
        cursor.filter(|&since| since > self.resync_floor && since <= self.value)
    }
}

/// Move the sync clock and its resync floor to `value` and stamp every live
/// recipe version, tag, shopping item and meal plan with it, so any client whose cursor
/// is at or below `value` gets a full resync (see [`SyncCursor::since`]).
/// For when the database's contents changed under clients' feet: an import,
/// a restored backup. `value` must be at or above every cursor a client could
/// hold. Call inside [`write`].
pub fn restamp_live(conn: &mut SqliteConnection, value: i64) -> QueryResult<()> {
    conn.batch_execute(&format!(
        "UPDATE sync_clock SET value = {value}, resync_floor = {value}; \
         UPDATE recipe_versions SET change_seq = {value} \
           WHERE id IN (SELECT current_version_id FROM recipes WHERE deleted_at IS NULL); \
         UPDATE user_tags SET change_seq = {value} WHERE deleted_at IS NULL; \
         UPDATE shopping_list_items SET change_seq = {value} WHERE deleted_at IS NULL; \
         UPDATE meal_plans SET change_seq = {value} WHERE deleted_at IS NULL;"
    ))
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
                            if is_database_locked(err) {
                                // A writer waited out busy_timeout on SQLite's
                                // file lock: something wrote outside `write`,
                                // or another process holds the file.
                                tracing::error!(parent: &span, error = %err, "database is locked");
                            } else {
                                tracing::warn!(parent: &span, error = %err, "query failed");
                            }
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

/// Whether `err` is SQLite's SQLITE_BUSY ("database is locked"), which
/// Diesel surfaces only as a message.
fn is_database_locked(err: &diesel::result::Error) -> bool {
    err.to_string().contains("database is locked")
}

fn tracing_instrumentation() -> Option<Box<dyn Instrumentation>> {
    Some(Box::new(TracingInstrumentation))
}

/// Open the database file (creating it and its directory if missing), set up
/// the connection, and run pending schema and data migrations, including
/// moving any photo bytes and large step outputs still in the database out
/// to files. WAL mode is a property of the file, so it's set here once rather
/// than per connection (switching it needs every other connection out of the
/// way).
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
    crate::photos::store::move_blobs_out(&mut conn)
        .expect("Failed to move photo bytes out of the database");
    crate::scraping::output_store::move_outputs_out(&mut conn)
        .expect("Failed to move step outputs out of the database");
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

/// A small pool on a fresh migrated database in `dir`, for tests.
#[cfg(test)]
pub fn test_pool(dir: &std::path::Path) -> DbPool {
    let url = dir.join("db.sqlite").to_str().unwrap().to_string();
    drop(open_migrated(&url));
    r2d2::Pool::builder()
        .max_size(4)
        .connection_customizer(Box::new(ConnectionSetup))
        .build(ConnectionManager::<SqliteConnection>::new(url))
        .expect("Failed to create test database pool")
}

/// Whether `table` in the database `conn` is open on has `column`.
pub fn has_column(conn: &mut SqliteConnection, table: &str, column: &str) -> QueryResult<bool> {
    use crate::files::pragmas::pragma_table_info as columns;
    let count: i64 = columns::table
        .filter(columns::arg.eq(table))
        .filter(columns::name.eq(column))
        .count()
        .get_result(conn)?;
    Ok(count > 0)
}

#[cfg(test)]
mod write_lock_tests {
    use super::*;

    #[test]
    fn recognizes_a_busy_writer() {
        let dir = std::env::temp_dir().join(format!("ramekin-db-lock-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let url = dir.join("db.sqlite").to_str().unwrap().to_string();
        let mut holder = SqliteConnection::establish(&url).unwrap();
        holder.batch_execute("BEGIN IMMEDIATE;").unwrap();
        let mut waiter = SqliteConnection::establish(&url).unwrap();
        let err = waiter
            .batch_execute("PRAGMA busy_timeout = 0; BEGIN IMMEDIATE;")
            .unwrap_err();
        assert!(is_database_locked(&err), "{err}");
        assert!(!is_database_locked(&diesel::result::Error::NotFound));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[cfg(test)]
mod sync_cursor_tests {
    use super::SyncCursor;

    const CURSOR: SyncCursor = SyncCursor {
        value: 100,
        resync_floor: 50,
    };

    #[test]
    fn a_cursor_this_database_issued_since_the_last_restamp_is_a_delta() {
        assert_eq!(CURSOR.since(Some(51)), Some(51));
        assert_eq!(CURSOR.since(Some(100)), Some(100));
    }

    #[test]
    fn no_cursor_is_a_full_resync() {
        assert_eq!(CURSOR.since(None), None);
    }

    #[test]
    fn a_cursor_from_before_a_restore_or_import_is_a_full_resync() {
        assert_eq!(CURSOR.since(Some(50)), None);
        assert_eq!(CURSOR.since(Some(1)), None);
    }

    #[test]
    fn a_cursor_from_another_database_is_a_full_resync() {
        assert_eq!(CURSOR.since(Some(101)), None);
    }
}

#[cfg(test)]
mod state_check_constraint_tests {
    //! The 2026-11-07 CHECK constraints migration, run over rows written
    //! before it.
    use super::*;

    const CHECKS_VERSION: &str = "20261107170000";

    /// The one text column `sql` selects, as `value`.
    fn values(conn: &mut SqliteConnection, sql: &str) -> Vec<String> {
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type = diesel::sql_types::Text)]
            value: String,
        }
        diesel::sql_query(sql)
            .load::<Row>(conn)
            .unwrap()
            .into_iter()
            .map(|row| row.value)
            .collect()
    }

    /// A database migrated up to just before the CHECK constraints.
    fn before_checks(dir: &std::path::Path) -> SqliteConnection {
        let url = dir.join("db.sqlite");
        let mut conn = SqliteConnection::establish(url.to_str().unwrap()).unwrap();
        setup_connection(&mut conn).unwrap();
        for migration in conn.pending_migrations(MIGRATIONS).unwrap() {
            if migration.name().version().to_string().as_str() >= CHECKS_VERSION {
                break;
            }
            conn.run_migration(&*migration).unwrap();
        }
        conn
    }

    #[test]
    fn existing_rows_are_backfilled_and_kept() {
        let dir = std::env::temp_dir().join(format!("ramekin-checks-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut conn = before_checks(&dir);
        conn.batch_execute(
            "INSERT INTO users (id, username, password_hash) VALUES ('u', 'u', '');
             INSERT INTO sessions (id, user_id, token_hash, expires_at, token_type) VALUES
                 ('s1', 'u', 'h1', '2099-01-01', 'session'),
                 ('s2', 'u', 'h2', '2099-01-01', 'mystery');
             INSERT INTO recipes (seq, id, user_id) VALUES (1, 'r', 'u'), (2, 'gone', 'u');
             INSERT INTO recipe_versions
                 (id, recipe_id, title, ingredients, instructions, version_source)
             VALUES ('v1', 'r', 'Soup', '[]', 'Cook.', 'scrape'),
                    ('v2', 'r', 'Soup', '[]', 'Cook.', 'auto_tag'),
                    ('v3', 'r', 'Soup', '[]', 'Cook.', 'mystery');
             UPDATE recipes SET current_version_id = 'v3' WHERE id = 'r';
             UPDATE recipes SET deleted_seq = 7 WHERE id = 'r';
             UPDATE recipes SET deleted_at = '2026-01-01' WHERE id = 'gone';
             INSERT INTO user_tags (id, user_id, name) VALUES ('t', 'u', 'soup');
             INSERT INTO recipe_version_tags (recipe_version_id, tag_id) VALUES ('v3', 't');
             INSERT INTO shopping_list_items (id, user_id, item, source_recipe_id)
             VALUES ('i', 'u', 'leeks', 'r');
             INSERT INTO scrape_jobs (id, user_id, url, kind)
             VALUES ('j', 'u', 'https://example.com', 'url_scrape');
             INSERT INTO step_outputs
                 (id, scrape_job_id, step_name, build_id, output, success, error, output_hash)
             VALUES ('o1', 'j', 'fetch_html', 'b', 'null', 1, NULL, 'abc'),
                    ('o2', 'j', 'extract_recipe', 'b', '{}', 1, NULL, ''),
                    ('o3', 'j', 'save_recipe', 'b', '{}', 0, NULL, ''),
                    ('o4', 'j', 'auto_tag', 'b', '{}', 1, 'odd', '');
             INSERT INTO shopping_list_history (id, user_id, undoes_id, undone_by_id) VALUES
                 ('h1', 'u', NULL, NULL),
                 ('h2', 'u', 'h1', NULL);",
        )
        .unwrap();

        conn.run_pending_migrations(MIGRATIONS).unwrap();

        // Foreign keys are back on, and no ON DELETE action ran while the
        // tables were rebuilt.
        let fk = "SELECT CAST(foreign_keys AS TEXT) AS value FROM pragma_foreign_keys";
        assert_eq!(values(&mut conn, fk), ["1"]);
        let tags = "SELECT tag_id AS value FROM recipe_version_tags";
        assert_eq!(values(&mut conn, tags), ["t"]);
        let sources = "SELECT source_recipe_id AS value FROM shopping_list_items";
        assert_eq!(values(&mut conn, sources), ["r"]);
        // The search index's rowids still match recipes.seq.
        let search = "SELECT CAST(rowid AS TEXT) AS value FROM recipe_search";
        assert_eq!(values(&mut conn, search), ["1"]);

        let sessions = "SELECT token_type AS value FROM sessions ORDER BY id";
        assert_eq!(values(&mut conn, sessions), ["session", "bookmarklet"]);
        let versions = "SELECT version_source || ' ' || COALESCE(normalize_ops, '-') AS value \
                        FROM recipe_versions ORDER BY id";
        assert_eq!(
            values(&mut conn, versions),
            ["scrape -", "normalize [\"auto_tag\"]", "migration -"]
        );
        let recipes = "SELECT id || ' ' || (deleted_seq IS NOT NULL) AS value \
                       FROM recipes ORDER BY seq";
        assert_eq!(values(&mut conn, recipes), ["r 0", "gone 1"]);
        let outputs = "SELECT id || ' ' || success || ' ' || COALESCE(error, '-') || ' ' \
                       || COALESCE(output_hash, '-') AS value FROM step_outputs ORDER BY id";
        assert_eq!(
            values(&mut conn, outputs),
            [
                "o1 1 - abc",
                "o2 1 - -",
                "o3 0 The step failed without an error message. -",
                "o4 1 - -",
            ]
        );
        let history = "SELECT id || ' ' || COALESCE(undoes_id, '-') || ' ' \
                       || COALESCE(undone_by_id, '-') AS value \
                       FROM shopping_list_history ORDER BY id";
        assert_eq!(values(&mut conn, history), ["h1 - h2", "h2 h1 -"]);

        // The constraints hold from now on.
        for bad in [
            "UPDATE sessions SET token_type = 'admin'",
            "UPDATE recipe_versions SET version_source = 'mystery'",
            "UPDATE recipes SET deleted_seq = 3 WHERE id = 'r'",
            "UPDATE step_outputs SET error = NULL WHERE id = 'o3'",
            "UPDATE step_outputs SET error = 'x' WHERE id = 'o2'",
            "UPDATE step_outputs SET output_hash = 'abc' WHERE id = 'o2'",
            "UPDATE shopping_list_history SET undoes_id = id WHERE id = 'h1'",
            "INSERT INTO shopping_list_history (id, user_id, undoes_id) VALUES ('h3', 'u', 'h1')",
            "UPDATE shopping_list_history SET undone_by_id = 'h1' WHERE id = 'h2'",
        ] {
            assert!(conn.batch_execute(bad).is_err(), "{bad}");
        }
        // A soft delete still stamps.
        conn.batch_execute("UPDATE recipes SET deleted_at = '2026-11-07' WHERE id = 'r'")
            .unwrap();
        let stamped = "SELECT id || ' ' || (deleted_seq IS NOT NULL) AS value \
                       FROM recipes WHERE id = 'r'";
        assert_eq!(values(&mut conn, stamped), ["r 1"]);
        drop(conn);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
