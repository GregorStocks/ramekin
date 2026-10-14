//! One-shot copy of a Postgres Ramekin database into a fresh SQLite file:
//! `ramekin-server import-postgres <postgres-url>`, with `DATABASE_URL`
//! naming the SQLite file. See doc/sqlite-cutover.md for the runbook.
//!
//! Everything is read in one repeatable-read snapshot and written in one
//! SQLite transaction, so a failure leaves nothing behind but an empty
//! schema. Soft-deleted rows come across like any other.
//!
//! Sync stamps: clients hold cursors that are Postgres snapshot watermarks,
//! all at or below the snapshot's xmax. The sync clock starts above that
//! xmax, and every live recipe version, tag and shopping item is restamped
//! above it, so each client's next delta sync re-delivers all of them (the
//! ingredient haystack's serialization changed from jsonb text to compact
//! JSON, so they need the new text anyway). Deletions keep their Postgres
//! stamps, which compare correctly against those cursors.

use crate::db;
use diesel::prelude::*;
use diesel::sql_types as sql;
use diesel::sqlite::{Sqlite, SqliteConnection};
use postgres::{Client, NoTls, Row};

/// How a Postgres column is read and stored.
#[derive(Clone, Copy)]
enum Kind {
    /// Text as is (text, varchar, citext, uuid, date).
    Text,
    /// A timestamptz, stored in the server's UTC text form.
    Timestamp,
    /// jsonb, or a uuid[] read as JSON: re-serialized the way the server
    /// writes JSON (compact, keys sorted).
    Json,
    Int,
    BigInt,
    Real,
    Bool,
    Bytes,
}

/// One column: its SQLite name, the Postgres expression it's read from, and
/// how to convert it.
struct Column {
    name: &'static str,
    source: &'static str,
    kind: Kind,
}

const fn col(name: &'static str, kind: Kind) -> Column {
    Column {
        name,
        source: name,
        kind,
    }
}

const fn from(name: &'static str, source: &'static str, kind: Kind) -> Column {
    Column { name, source, kind }
}

struct Table {
    name: &'static str,
    columns: &'static [Column],
}

use Kind::*;

/// Tables in an order that satisfies their foreign keys.
/// `recipes.current_version_id` is set after the versions exist.
const TABLES: &[Table] = &[
    Table {
        name: "users",
        columns: &[
            col("id", Text),
            col("username", Text),
            col("password_hash", Text),
            col("created_at", Timestamp),
            col("updated_at", Timestamp),
            col("deleted_at", Timestamp),
        ],
    },
    Table {
        name: "sessions",
        columns: &[
            col("id", Text),
            col("user_id", Text),
            col("token_hash", Text),
            col("expires_at", Timestamp),
            col("created_at", Timestamp),
            col("token_type", Text),
        ],
    },
    Table {
        name: "client_log_uploads",
        columns: &[
            col("id", Text),
            col("user_id", Text),
            col("platform", Text),
            col("app_version", Text),
            col("os_info", Text),
            col("content", Text),
            col("created_at", Timestamp),
            col("deleted_at", Timestamp),
        ],
    },
    Table {
        name: "data_migrations",
        columns: &[col("name", Text), col("run_at", Timestamp)],
    },
    Table {
        name: "ingredient_name_resolutions",
        columns: &[
            col("name", Text),
            col("status", Text),
            col("disposition", Text),
            col("catalog_key", Text),
            col("model", Text),
            col("error", Text),
            col("attempts", Int),
            col("created_at", Timestamp),
            col("updated_at", Timestamp),
            col("kcal_per_100g", Real),
            col("grams_per_cup", Real),
            col("grams_per_piece", Real),
        ],
    },
    Table {
        name: "photos",
        columns: &[
            col("id", Text),
            col("user_id", Text),
            col("content_type", Text),
            col("data", Bytes),
            col("created_at", Timestamp),
            col("deleted_at", Timestamp),
            col("thumbnail", Bytes),
            col("width", Int),
            col("height", Int),
            col("file_size", Int),
        ],
    },
    Table {
        name: "photo_thumbnails",
        columns: &[
            col("id", Text),
            col("photo_id", Text),
            col("size", Int),
            col("data", Bytes),
            col("created_at", Timestamp),
        ],
    },
    Table {
        name: "recipes",
        columns: &[
            col("id", Text),
            col("user_id", Text),
            col("created_at", Timestamp),
            col("deleted_at", Timestamp),
            from("deleted_seq", "deleted_xid", BigInt),
        ],
    },
    Table {
        name: "recipe_versions",
        columns: &[
            col("id", Text),
            col("recipe_id", Text),
            col("title", Text),
            col("description", Text),
            col("ingredients", Json),
            col("instructions", Text),
            col("source_url", Text),
            col("source_name", Text),
            from("photo_ids", "array_to_json(photo_ids)", Json),
            col("servings", Text),
            col("prep_time", Text),
            col("cook_time", Text),
            col("total_time", Text),
            col("rating", Int),
            col("difficulty", Text),
            col("nutritional_info", Text),
            col("notes", Text),
            col("version_source", Text),
            col("created_at", Timestamp),
        ],
    },
    Table {
        name: "user_tags",
        columns: &[
            col("id", Text),
            col("user_id", Text),
            col("name", Text),
            col("created_at", Timestamp),
            col("deleted_at", Timestamp),
            col("updated_at", Timestamp),
        ],
    },
    Table {
        name: "recipe_version_tags",
        columns: &[col("recipe_version_id", Text), col("tag_id", Text)],
    },
    Table {
        name: "meal_plans",
        columns: &[
            col("id", Text),
            col("user_id", Text),
            col("recipe_id", Text),
            col("meal_date", Text),
            col("meal_type", Text),
            col("notes", Text),
            col("created_at", Timestamp),
            col("deleted_at", Timestamp),
        ],
    },
    Table {
        name: "shopping_list_items",
        columns: &[
            col("id", Text),
            col("user_id", Text),
            col("item", Text),
            col("amount", Text),
            col("note", Text),
            col("source_recipe_id", Text),
            col("source_recipe_title", Text),
            col("is_checked", Bool),
            col("sort_order", Int),
            col("created_at", Timestamp),
            col("updated_at", Timestamp),
            col("client_id", Text),
            col("version", Int),
            col("deleted_at", Timestamp),
            col("category_override", Text),
            col("computed_category", Text),
        ],
    },
    Table {
        name: "scrape_jobs",
        columns: &[
            col("id", Text),
            col("user_id", Text),
            col("url", Text),
            col("status", Text),
            col("failed_at_step", Text),
            col("recipe_id", Text),
            col("error_message", Text),
            col("retry_count", Int),
            col("created_at", Timestamp),
            col("updated_at", Timestamp),
            col("current_step", Text),
            col("photo_only", Bool),
            col("current_step_started_at", Timestamp),
            col("expected_version_id", Text),
            col("idempotency_key", Text),
        ],
    },
    Table {
        name: "step_outputs",
        columns: &[
            col("id", Text),
            col("scrape_job_id", Text),
            col("step_name", Text),
            col("build_id", Text),
            col("output", Json),
            col("created_at", Timestamp),
            col("duration_ms", BigInt),
            col("summary", Text),
            col("success", Bool),
            col("error", Text),
        ],
    },
];

/// The Postgres expression for a column, cast so it reads as `kind` expects.
fn select_expr(column: &Column) -> String {
    let source = column.source;
    match column.kind {
        // Microseconds, the precision Postgres stores.
        Timestamp => {
            format!("to_char({source} AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS.US') || '+00:00'")
        }
        Text | Json => format!("({source})::text"),
        Int | BigInt | Real | Bool | Bytes => source.to_string(),
    }
}

/// A non-null value read from Postgres, ready to bind into SQLite.
enum Value {
    Text(String),
    Int(i64),
    Real(f64),
    Bytes(Vec<u8>),
}

fn read(row: &Row, index: usize, column: &Column) -> anyhow::Result<Option<Value>> {
    Ok(match column.kind {
        Text | Timestamp => row.try_get::<_, Option<String>>(index)?.map(Value::Text),
        Json => match row.try_get::<_, Option<String>>(index)? {
            Some(s) => {
                let parsed: serde_json::Value = serde_json::from_str(&s)?;
                Some(Value::Text(serde_json::to_string(&parsed)?))
            }
            None => None,
        },
        Int => row
            .try_get::<_, Option<i32>>(index)?
            .map(|v| Value::Int(v.into())),
        BigInt => row.try_get::<_, Option<i64>>(index)?.map(Value::Int),
        Real => row.try_get::<_, Option<f64>>(index)?.map(Value::Real),
        Bool => row
            .try_get::<_, Option<bool>>(index)?
            .map(|v| Value::Int(v.into())),
        Bytes => row.try_get::<_, Option<Vec<u8>>>(index)?.map(Value::Bytes),
    })
}

fn bind<'f, Q>(
    query: diesel::query_builder::BoxedSqlQuery<'f, Sqlite, Q>,
    value: Option<Value>,
) -> diesel::query_builder::BoxedSqlQuery<'f, Sqlite, Q> {
    match value {
        None => query.bind::<sql::Nullable<sql::Text>, _>(None::<String>),
        Some(Value::Text(v)) => query.bind::<sql::Text, _>(v),
        Some(Value::Int(v)) => query.bind::<sql::BigInt, _>(v),
        Some(Value::Real(v)) => query.bind::<sql::Double, _>(v),
        Some(Value::Bytes(v)) => query.bind::<sql::Binary, _>(v),
    }
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = sql::BigInt)]
    count: i64,
}

fn sqlite_count(conn: &mut SqliteConnection, table: &str) -> QueryResult<i64> {
    // Table names come from the fixed list above.
    diesel::sql_query(format!("SELECT count(*) AS count FROM {table}"))
        .get_result::<Count>(conn)
        .map(|c| c.count)
}

fn copy_table(
    pg: &mut postgres::Transaction<'_>,
    conn: &mut SqliteConnection,
    table: &Table,
) -> anyhow::Result<usize> {
    let selects: Vec<String> = table.columns.iter().map(select_expr).collect();
    let names: Vec<&str> = table.columns.iter().map(|c| c.name).collect();
    let insert = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        table.name,
        names.join(", "),
        vec!["?"; names.len()].join(", ")
    );

    // Streamed: photo tables hold every image, too much to load at once.
    let mut rows = pg.query_raw(
        &format!("SELECT {} FROM {}", selects.join(", "), table.name),
        std::iter::empty::<i32>(),
    )?;
    let mut count = 0;
    while let Some(row) = postgres::fallible_iterator::FallibleIterator::next(&mut rows)? {
        let mut query = diesel::sql_query(insert.as_str()).into_boxed::<Sqlite>();
        for (index, column) in table.columns.iter().enumerate() {
            query = bind(query, read(&row, index, column)?);
        }
        query.execute(conn)?;
        count += 1;
    }
    Ok(count)
}

/// Copy the Postgres database at `postgres_url` into the empty SQLite
/// database at `database_url`.
pub fn run(postgres_url: &str, database_url: &str) -> anyhow::Result<()> {
    let mut conn = db::open_migrated(database_url);
    let existing = sqlite_count(&mut conn, "users")? + sqlite_count(&mut conn, "recipes")?;
    anyhow::ensure!(
        existing == 0,
        "{database_url} already has data; import into a new file"
    );

    let mut pg = Client::connect(postgres_url, NoTls)?;
    let mut snapshot = pg
        .build_transaction()
        .isolation_level(postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()?;
    // In the same snapshot as every read below: above every cursor a client
    // could hold and every change it could have seen.
    let xmax: i64 = snapshot
        .query_one(
            "SELECT pg_snapshot_xmax(pg_current_snapshot())::text::bigint",
            &[],
        )?
        .get(0);

    // Validated before the transaction commits, so a failed check leaves
    // nothing behind.
    let indexed = conn.immediate_transaction(|conn| {
        let mut copied = Vec::new();
        for table in TABLES {
            let count = copy_table(&mut snapshot, conn, table)?;
            tracing::info!(table = table.name, rows = count, "copied");
            copied.push((table.name, count));
        }

        // Point each recipe at its current version. The search index
        // triggers build each live recipe's index row from it.
        for row in snapshot.query(
            "SELECT id::text, current_version_id::text FROM recipes \
             WHERE current_version_id IS NOT NULL",
            &[],
        )? {
            diesel::sql_query("UPDATE recipes SET current_version_id = ? WHERE id = ?")
                .bind::<sql::Text, _>(row.get::<_, String>(1))
                .bind::<sql::Text, _>(row.get::<_, String>(0))
                .execute(conn)?;
        }

        // Start the clock above every Postgres stamp first, so a trigger
        // that fires during the fix-ups below stamps above it too.
        let restamp = xmax + 1;
        diesel::sql_query("UPDATE sync_clock SET value = ?")
            .bind::<sql::BigInt, _>(restamp)
            .execute(conn)?;

        // Deleted tags and shopping items keep their Postgres stamps.
        for (table, stamp) in [
            ("user_tags", "change_xid"),
            ("shopping_list_items", "change_xid"),
        ] {
            for row in snapshot.query(
                &format!("SELECT id::text, {stamp} FROM {table} WHERE deleted_at IS NOT NULL"),
                &[],
            )? {
                diesel::sql_query(format!("UPDATE {table} SET change_seq = ? WHERE id = ?"))
                    .bind::<sql::BigInt, _>(row.get::<_, i64>(1))
                    .bind::<sql::Text, _>(row.get::<_, String>(0))
                    .execute(conn)?;
            }
        }

        // Everything live, above every client's cursor.
        db::restamp_live(conn, restamp)?;

        // Every row came across, deleted ones included.
        for (table, count) in &copied {
            let sqlite = sqlite_count(conn, table)?;
            anyhow::ensure!(
                sqlite == *count as i64,
                "{table}: {count} rows in Postgres, {sqlite} in SQLite"
            );
        }
        let live: i64 = snapshot
            .query_one(
                "SELECT count(*) FROM recipes WHERE deleted_at IS NULL AND current_version_id IS NOT NULL",
                &[],
            )?
            .get(0);
        let indexed = sqlite_count(conn, "recipe_search")?;
        anyhow::ensure!(
            indexed == live,
            "search index has {indexed} rows for {live} live recipes"
        );

        anyhow::Ok(indexed)
    })?;
    // The snapshot only read; dropping it rolls it back.
    drop(snapshot);

    tracing::info!(xmax, indexed, "import complete");
    Ok(())
}
