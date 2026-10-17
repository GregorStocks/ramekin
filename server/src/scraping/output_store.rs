//! Database-backed step output store for the server.
//!
//! An output whose JSON is larger than [`INLINE_LIMIT`] (in practice, a
//! fetched page) lives in the file store beside the database
//! ([`crate::files`]), zstd-compressed: its row holds JSON null in `output`
//! and names the file in `output_hash`. Smaller outputs stay inline. Files
//! written before compression, uncompressed, still load.

use std::collections::{HashMap, HashSet};
use std::error::Error;

use crate::ids::Uuid;
use anyhow::Context;
use async_trait::async_trait;
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::SqliteConnection;
use ramekin_core::pipeline::StepOutputStore;
use ramekin_core::BUILD_ID;
use serde_json::Value as JsonValue;

use crate::db::{self, run_blocking, DbPool};
use crate::files::{self, FileStore};
use crate::models::NewStepOutput;
use crate::schema::step_outputs;
use crate::scraping::status::step_summary;

/// Outputs whose JSON is longer than this many bytes go to the file store.
const INLINE_LIMIT: usize = 16 * 1024;

/// zstd level for stored outputs. It shrinks prod's fetched pages 6.9x at
/// about 10 ms a page; level 3 manages 6.3x, and 15 barely beats 9.
const ZSTD_LEVEL: i32 = 9;

/// Write `json` to `store` compressed, returning the file's hash.
fn put(store: &FileStore, json: &[u8]) -> anyhow::Result<String> {
    store.put(&zstd::bulk::compress(json, ZSTD_LEVEL)?)
}

/// A step output ready to insert: its summary, and the output itself either
/// inline or already written to the file store. Prepare it before the write
/// transaction that inserts it, since writing the file waits on the disk.
pub struct PreparedOutput {
    summary: Option<String>,
    output: JsonValue,
    output_hash: String,
}

impl PreparedOutput {
    /// Prepare `output` of step `step_name`, writing it to `store` if it's
    /// large.
    pub fn new(store: &FileStore, step_name: &str, output: JsonValue) -> anyhow::Result<Self> {
        let summary = step_summary(step_name, &output);
        let json = serde_json::to_vec(&output)?;
        if json.len() <= INLINE_LIMIT {
            return Ok(Self {
                summary,
                output,
                output_hash: String::new(),
            });
        }
        Ok(Self {
            summary,
            output: JsonValue::Null,
            output_hash: put(store, &json)?,
        })
    }

    /// [`PreparedOutput::new`] against the server's store, off the async
    /// runtime.
    pub async fn prepare(step_name: &str, output: JsonValue) -> anyhow::Result<Self> {
        let step_name = step_name.to_string();
        tokio::task::spawn_blocking(move || Self::new(files::live(), &step_name, output)).await?
    }

    /// The row recording this output of step `step_name` in job `job_id`.
    pub fn into_row(
        self,
        job_id: Uuid,
        step_name: &str,
        duration_ms: Option<i64>,
        success: bool,
        error: Option<&str>,
    ) -> NewStepOutput {
        NewStepOutput {
            id: Uuid::now_v7(),
            scrape_job_id: job_id,
            step_name: step_name.to_string(),
            build_id: BUILD_ID.to_string(),
            output: self.output,
            duration_ms,
            summary: self.summary,
            success,
            error: error.map(|s| s.to_string()),
            output_hash: self.output_hash,
        }
    }
}

/// The output a row holds: `output` itself, or the file `output_hash` names.
fn load(store: &FileStore, output: JsonValue, output_hash: &str) -> anyhow::Result<JsonValue> {
    if output_hash.is_empty() {
        return Ok(output);
    }
    let bytes = store.read(output_hash)?;
    // Files from before compression hold the JSON as is, and JSON never
    // starts with zstd's magic number.
    let json = if bytes.starts_with(&zstd::zstd_safe::MAGICNUMBER.to_le_bytes()) {
        zstd::stream::decode_all(bytes.as_slice())
            .with_context(|| format!("stored output {output_hash} isn't valid zstd"))?
    } else {
        bytes
    };
    serde_json::from_slice(&json).with_context(|| format!("stored output {output_hash} isn't JSON"))
}

/// The latest output of step `step_name` in job `job_id`, or only of its
/// successful runs if `successful`. Its file, if it has one, is read after
/// the database connection is handed back.
pub async fn latest_output(
    pool: &DbPool,
    job_id: Uuid,
    step_name: &str,
    successful: bool,
) -> anyhow::Result<Option<JsonValue>> {
    let step_name = step_name.to_string();
    let row: Option<(JsonValue, String)> = run_blocking(pool, move |conn| {
        let mut query = step_outputs::table
            .filter(step_outputs::scrape_job_id.eq(job_id))
            .filter(step_outputs::step_name.eq(step_name))
            .into_boxed();
        if successful {
            query = query.filter(step_outputs::success.eq(true));
        }
        query
            .order(step_outputs::created_at.desc())
            .select((step_outputs::output, step_outputs::output_hash))
            .first(conn)
            .optional()
    })
    .await??;
    let Some((output, output_hash)) = row else {
        return Ok(None);
    };
    let output =
        tokio::task::spawn_blocking(move || load(files::live(), output, &output_hash)).await??;
    Ok(Some(output))
}

diesel::define_sql_function! {
    /// SQLite length(): a text's length in characters.
    fn length(value: diesel::sql_types::Json) -> diesel::sql_types::BigInt;
}

/// Rows per batch when moving inline outputs out. A batch's outputs are all
/// in memory at once (a fetched page runs to a few hundred KB), and its write
/// transaction holds up every other writer.
const MOVE_BATCH: i64 = 32;

/// Move inline outputs larger than [`INLINE_LIMIT`] out to files: ones
/// written before large outputs went to files, in this database or a
/// snapshot restored into it. Each batch is written to files first, then its
/// rows cleared in one short transaction, so an interrupted run resumes
/// where it stopped. If anything moved, VACUUM then hands the space back to
/// the file system. One query once nothing is left to move.
pub fn move_outputs_out(conn: &mut SqliteConnection) -> anyhow::Result<()> {
    let store = FileStore::of(conn)?;
    let mut moved = 0;
    loop {
        // Characters, not bytes, but a text is never shorter in bytes.
        let batch: Vec<(Uuid, JsonValue)> = step_outputs::table
            .filter(step_outputs::output_hash.eq(""))
            .filter(length(step_outputs::output).gt(INLINE_LIMIT as i64))
            .select((step_outputs::id, step_outputs::output))
            .limit(MOVE_BATCH)
            .load(conn)?;
        if batch.is_empty() {
            break;
        }
        let stored = batch
            .into_iter()
            .map(|(id, output)| Ok((id, put(&store, &serde_json::to_vec(&output)?)?)))
            .collect::<anyhow::Result<Vec<_>>>()?;
        db::write(conn, |conn| {
            for (id, output_hash) in &stored {
                diesel::update(step_outputs::table.find(id))
                    .set((
                        step_outputs::output.eq(JsonValue::Null),
                        step_outputs::output_hash.eq(output_hash),
                    ))
                    .execute(conn)?;
            }
            QueryResult::Ok(())
        })?;
        moved += stored.len();
    }
    if moved > 0 {
        conn.batch_execute("VACUUM; PRAGMA wal_checkpoint(TRUNCATE);")?;
        tracing::info!(moved, dir = %store.dir().display(), "moved large step outputs out of the database");
    }
    Ok(())
}

/// Database-backed output store for server pipeline runs.
///
/// Stores step outputs in the step_outputs table, keyed by job_id and step_name.
pub struct DbOutputStore<'a> {
    pool: &'a DbPool,
    job_id: Uuid,
}

impl<'a> DbOutputStore<'a> {
    /// Create a new database output store for a job.
    pub fn new(pool: &'a DbPool, job_id: Uuid) -> Self {
        Self { pool, job_id }
    }

    /// Steps whose latest output row in this job is a success.
    pub async fn succeeded_steps(&self) -> Result<HashSet<String>, Box<dyn Error + Send + Sync>> {
        let job_id = self.job_id;
        let rows: Vec<(String, bool)> = run_blocking(self.pool, move |conn| {
            step_outputs::table
                .filter(step_outputs::scrape_job_id.eq(job_id))
                .order(step_outputs::created_at.asc())
                .select((step_outputs::step_name, step_outputs::success))
                .load(conn)
        })
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        let mut latest = HashMap::new();
        for (name, success) in rows {
            latest.insert(name, success);
        }
        Ok(latest
            .into_iter()
            .filter_map(|(name, success)| success.then_some(name))
            .collect())
    }

    /// Insert a step output row. Takes `&self` so concurrently running steps
    /// can keep reading the store while finished ones are saved.
    pub async fn save(
        &self,
        step_name: &str,
        output: &JsonValue,
        duration_ms: i64,
        success: bool,
        error: Option<&str>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let new_output = PreparedOutput::prepare(step_name, output.clone())
            .await
            .map_err(|e| Box::<dyn Error + Send + Sync>::from(format!("{e:#}")))?
            .into_row(self.job_id, step_name, Some(duration_ms), success, error);

        crate::db::run_blocking_write(self.pool, move |conn| {
            diesel::insert_into(step_outputs::table)
                .values(&new_output)
                .execute(conn)
        })
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        Ok(())
    }
}

#[async_trait]
impl StepOutputStore for DbOutputStore<'_> {
    async fn get_output(&self, step_name: &str) -> Option<JsonValue> {
        latest_output(self.pool, self.job_id, step_name, false)
            .await
            .unwrap_or_else(|e| {
                tracing::error!(job_id = %self.job_id, step_name, "failed to load step output: {e:#}");
                None
            })
    }

    async fn save_output(
        &mut self,
        step_name: &str,
        output: &JsonValue,
        duration_ms: i64,
        success: bool,
        error: Option<&str>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.save(step_name, output, duration_ms, success, error)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::sql_types::Text;

    fn page(i: usize) -> JsonValue {
        serde_json::json!({ "html": format!("<p>soup {i}</p>").repeat(INLINE_LIMIT / 10) })
    }

    #[test]
    fn large_outputs_go_to_files_and_small_ones_stay_inline() {
        let dir = files::temp_dir("output-store-test");
        let store = FileStore::new(dir.join("files"));

        let small = serde_json::json!({ "recipe_id": "r" });
        let prepared = PreparedOutput::new(&store, "save_recipe", small.clone()).unwrap();
        assert_eq!(prepared.output, small);
        assert_eq!(prepared.output_hash, "");
        assert_eq!(
            load(&store, prepared.output, &prepared.output_hash).unwrap(),
            small
        );

        let large = page(0);
        let prepared = PreparedOutput::new(&store, "fetch_html", large.clone()).unwrap();
        assert_eq!(prepared.output, JsonValue::Null);
        assert_ne!(prepared.output_hash, "");
        let stored = store.read(&prepared.output_hash).unwrap();
        assert!(stored.len() < serde_json::to_vec(&large).unwrap().len() / 2);
        assert_eq!(
            load(&store, prepared.output, &prepared.output_hash).unwrap(),
            large
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn uncompressed_output_files_still_load() {
        let dir = files::temp_dir("output-raw-test");
        let store = FileStore::new(dir.join("files"));
        let large = page(0);
        let hash = store.put(&serde_json::to_vec(&large).unwrap()).unwrap();
        assert_eq!(load(&store, JsonValue::Null, &hash).unwrap(), large);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn large_inline_outputs_move_to_files_at_startup() {
        let dir = files::temp_dir("output-move-test");
        let database = dir.join("db.sqlite");
        let mut conn = db::open_migrated(database.to_str().unwrap());
        diesel::sql_query("PRAGMA foreign_keys = OFF")
            .execute(&mut conn)
            .unwrap();
        // More than a batch of large outputs written inline, as they were
        // before files, and a small one.
        let insert = |conn: &mut SqliteConnection, id: String, output: &JsonValue| {
            diesel::sql_query(
                "INSERT INTO step_outputs (id, scrape_job_id, step_name, build_id, output) \
                 VALUES (?, '00000000-0000-0000-0000-000000000000', 'fetch_html', 'b', ?)",
            )
            .bind::<Text, _>(id)
            .bind::<Text, _>(output.to_string())
            .execute(conn)
            .unwrap();
        };
        let count = MOVE_BATCH as usize + 3;
        for i in 0..count {
            insert(
                &mut conn,
                format!("00000000-0000-0000-0000-{i:012}"),
                &page(i),
            );
        }
        let small = serde_json::json!({ "photo_ids": [] });
        insert(
            &mut conn,
            "ffffffff-0000-0000-0000-000000000000".into(),
            &small,
        );

        move_outputs_out(&mut conn).unwrap();

        let store = FileStore::beside(&database);
        let rows: Vec<(Uuid, JsonValue, String)> = step_outputs::table
            .order(step_outputs::id)
            .select((
                step_outputs::id,
                step_outputs::output,
                step_outputs::output_hash,
            ))
            .load(&mut conn)
            .unwrap();
        assert_eq!(rows.len(), count + 1);
        for (i, (_, output, output_hash)) in rows.iter().take(count).enumerate() {
            assert_eq!(*output, JsonValue::Null);
            assert_eq!(load(&store, output.clone(), output_hash).unwrap(), page(i));
        }
        let (_, output, output_hash) = rows.last().unwrap();
        assert_eq!((output, output_hash.as_str()), (&small, ""));

        // Run again, as every later startup does: nothing to do.
        move_outputs_out(&mut conn).unwrap();
        drop(conn);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
