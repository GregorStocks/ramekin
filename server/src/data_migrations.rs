//! Data migrations written in Rust, for changes SQL can't express without
//! duplicating server logic (anything that needs the ingredient parser or
//! catalog) or that rewrite a big table. Each runs once, at startup after the
//! SQL migrations, and a failure stops startup. A [`DataMigration::Once`] runs
//! in one transaction together with the `data_migrations` row that records it;
//! a [`DataMigration::Batched`] one commits batch by batch, so it never holds
//! the write lock (or grows the WAL) for a whole table at once.
//!
//! Migrations use the current models and Diesel DSL, so one that has run in
//! production can be deleted once it no longer compiles; its row stays.

use crate::ids::Uuid;
use crate::schema::{data_migrations, step_outputs};
use crate::sql_types::{Compressed, CompressedJson};
use anyhow::Context;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use std::collections::HashSet;

enum DataMigration {
    /// Runs in one transaction.
    #[allow(dead_code)] // No pending migration needs it today.
    Once(fn(&mut SqliteConnection) -> anyhow::Result<()>),
    /// Runs one batch per transaction and returns whether any work remains.
    /// A batch must pick up where the last committed one left off, since a
    /// crash between batches reruns the migration from the top.
    Batched(fn(&mut SqliteConnection) -> anyhow::Result<bool>),
}

/// Every data migration, in the order they run. Never rename one that has run.
/// (`strip_materialized_grams` and `promote_section_headers` ran on Postgres
/// before the SQLite move; their rows came across with the data.)
const DATA_MIGRATIONS: &[(&str, DataMigration)] = &[(
    "compress_step_outputs",
    DataMigration::Batched(compress_step_outputs),
)];

/// Run each data migration that hasn't run yet.
pub fn run_pending(conn: &mut SqliteConnection) -> anyhow::Result<()> {
    let done: HashSet<String> = data_migrations::table
        .select(data_migrations::name)
        .load(conn)
        .context("failed to load completed data migrations")?
        .into_iter()
        .collect();
    for (name, migration) in DATA_MIGRATIONS {
        if done.contains(*name) {
            continue;
        }
        match migration {
            DataMigration::Once(migrate) => crate::db::write(conn, |conn| {
                migrate(conn)?;
                record(conn, name)
            }),
            DataMigration::Batched(batch) => loop {
                let more = crate::db::write(conn, |conn| {
                    let more = batch(conn)?;
                    if !more {
                        record(conn, name)?;
                    }
                    anyhow::Ok(more)
                });
                match more {
                    Ok(true) => continue,
                    Ok(false) => break Ok(()),
                    Err(e) => break Err(e),
                }
            },
        }
        .with_context(|| format!("data migration {name} failed"))?;
        tracing::info!(name, "ran data migration");
    }
    Ok(())
}

fn record(conn: &mut SqliteConnection, name: &str) -> anyhow::Result<()> {
    diesel::insert_into(data_migrations::table)
        .values(data_migrations::name.eq(name))
        .execute(conn)?;
    Ok(())
}

diesel::define_sql_function! {
    /// SQLite typeof(): the storage class of a value, such as 'text' or 'blob'.
    #[sql_name = "typeof"]
    fn storage_class(value: CompressedJson) -> diesel::sql_types::Text;
}

/// Rows per batch of [`compress_step_outputs`]. A fetched page runs to a few
/// hundred KB, so a batch is tens of MB to hold and compress.
const COMPRESS_BATCH_SIZE: i64 = 100;

/// Rewrite step outputs written as plain JSON text before `output` became
/// [`CompressedJson`], so the rewrite stores them compressed.
fn compress_step_outputs(conn: &mut SqliteConnection) -> anyhow::Result<bool> {
    let rows: Vec<(Uuid, serde_json::Value)> = step_outputs::table
        .filter(storage_class(step_outputs::output).eq("text"))
        .select((step_outputs::id, step_outputs::output))
        .limit(COMPRESS_BATCH_SIZE)
        .load(conn)?;
    let more = rows.len() as i64 == COMPRESS_BATCH_SIZE;
    for (id, output) in rows {
        diesel::update(step_outputs::table.find(id))
            .set(step_outputs::output.eq(Compressed(output)))
            .execute(conn)?;
    }
    Ok(more)
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::sql_types::Text;

    #[derive(QueryableByName)]
    struct StorageClass {
        #[diesel(sql_type = Text)]
        class: String,
    }

    fn storage_classes(conn: &mut SqliteConnection) -> Vec<String> {
        diesel::sql_query("SELECT DISTINCT typeof(output) AS class FROM step_outputs")
            .load::<StorageClass>(conn)
            .unwrap()
            .into_iter()
            .map(|row| row.class)
            .collect()
    }

    fn outputs(conn: &mut SqliteConnection) -> Vec<serde_json::Value> {
        step_outputs::table
            .order(step_outputs::id)
            .select(step_outputs::output)
            .load(conn)
            .unwrap()
    }

    #[test]
    fn compress_step_outputs_rewrites_text_rows_in_batches() {
        let dir =
            std::env::temp_dir().join(format!("ramekin-compress-test-{}", uuid::Uuid::new_v4()));
        let path = dir.join("db.sqlite");
        let mut conn = crate::db::open_migrated(path.to_str().unwrap());
        diesel::sql_query("PRAGMA foreign_keys = OFF")
            .execute(&mut conn)
            .unwrap();

        // More than a batch of rows written as plain JSON text, the way
        // they were stored before compression.
        let count = COMPRESS_BATCH_SIZE as usize * 2 + 5;
        let expected: Vec<serde_json::Value> = (0..count)
            .map(|i| serde_json::json!({ "html": "<p>soup</p>".repeat(100), "n": i }))
            .collect();
        for (i, output) in expected.iter().enumerate() {
            diesel::sql_query(
                "INSERT INTO step_outputs (id, scrape_job_id, step_name, build_id, output) \
                 VALUES (?, 'job', 'fetch_html', 'build', ?)",
            )
            .bind::<Text, _>(format!("00000000-0000-0000-0000-{i:012}"))
            .bind::<Text, _>(output.to_string())
            .execute(&mut conn)
            .unwrap();
        }
        assert_eq!(storage_classes(&mut conn), vec!["text"]);
        // Text rows read back before they're compressed.
        assert_eq!(outputs(&mut conn), expected);

        diesel::delete(
            data_migrations::table.filter(data_migrations::name.eq("compress_step_outputs")),
        )
        .execute(&mut conn)
        .unwrap();
        run_pending(&mut conn).unwrap();

        assert_eq!(storage_classes(&mut conn), vec!["blob"]);
        assert_eq!(outputs(&mut conn), expected);
        let recorded: i64 = data_migrations::table
            .filter(data_migrations::name.eq("compress_step_outputs"))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(recorded, 1);
        drop(conn);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
