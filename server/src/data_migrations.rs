//! Data migrations written in Rust, for changes SQL can't express without
//! duplicating server logic (anything that needs the ingredient parser or
//! catalog). Each runs once, at startup after the SQL migrations, in its own
//! transaction together with the `data_migrations` row that records it, and a
//! failure stops startup.
//!
//! Migrations use the current models and Diesel DSL, so one that has run in
//! production can be deleted once it no longer compiles; its row stays.

use crate::schema::data_migrations;
use anyhow::Context;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use std::collections::HashSet;

type DataMigration = fn(&mut SqliteConnection) -> anyhow::Result<()>;

/// Every data migration, in the order they run. Never rename one that has run.
/// (`strip_materialized_grams` and `promote_section_headers` ran on Postgres
/// before the SQLite move; their rows came across with the data.)
const DATA_MIGRATIONS: &[(&str, DataMigration)] = &[];

/// Run each data migration that hasn't run yet.
pub fn run_pending(conn: &mut SqliteConnection) -> anyhow::Result<()> {
    let done: HashSet<String> = data_migrations::table
        .select(data_migrations::name)
        .load(conn)
        .context("failed to load completed data migrations")?
        .into_iter()
        .collect();
    for (name, migrate) in DATA_MIGRATIONS {
        if done.contains(*name) {
            continue;
        }
        crate::db::write(conn, |conn| {
            migrate(conn)?;
            diesel::insert_into(data_migrations::table)
                .values(data_migrations::name.eq(name))
                .execute(conn)?;
            anyhow::Ok(())
        })
        .with_context(|| format!("data migration {name} failed"))?;
        tracing::info!(name, "ran data migration");
    }
    Ok(())
}
