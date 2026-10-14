//! Scheduled snapshots of the SQLite database into `RAMEKIN_BACKUP_DIR`,
//! and `ramekin-server backup <dir>` for one on demand. See the Backups
//! section of doc/sqlite-cutover.md.
//!
//! A snapshot is `VACUUM INTO`: a consistent, compacted, self-contained copy
//! (no WAL) taken in a read transaction, so writers carry on meanwhile. It's
//! written under a temporary name and renamed into place, so a sync tool
//! watching the directory never picks up a partial file.

use crate::db::{self, DbPool};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::sql_types::Text;
use diesel::SqliteConnection;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const PREFIX: &str = "ramekin-";
/// Where a snapshot is written before it's renamed into place.
const PARTIAL: &str = ".ramekin-snapshot.tmp";
const SUFFIX: &str = ".sqlite";
/// UTC, sortable, and safe in file names on every platform.
const STAMP_FORMAT: &str = "%Y-%m-%dT%H-%M-%SZ";
/// One snapshot a day...
const INTERVAL: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);
/// How soon a failed snapshot is tried again.
const RETRY: std::time::Duration = std::time::Duration::from_secs(60 * 60);
/// Each snapshot is kept this long.
const KEEP_DAYS: i64 = 7;

fn file_name(at: DateTime<Utc>) -> String {
    format!("{PREFIX}{}{SUFFIX}", at.format(STAMP_FORMAT))
}

/// When the snapshot named `name` was taken, or None for any other file.
fn taken_at(name: &str) -> Option<DateTime<Utc>> {
    let stamp = name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
    NaiveDateTime::parse_from_str(stamp, STAMP_FORMAT)
        .ok()
        .map(|naive| naive.and_utc())
}

/// Write a snapshot of the database `conn` is open on into `dir`, named for
/// `now`. Returns its path.
pub fn snapshot(
    conn: &mut SqliteConnection,
    dir: &Path,
    now: DateTime<Utc>,
) -> anyhow::Result<PathBuf> {
    let path = dir.join(file_name(now));
    // One fixed temporary name: VACUUM INTO refuses to overwrite a file, and
    // whatever a crashed or failed attempt left there is incomplete, so each
    // attempt clears it rather than leaving one behind per attempt.
    let partial = dir.join(PARTIAL);
    if partial.exists() {
        std::fs::remove_file(&partial)?;
    }
    let target = partial
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("backup path {} is not UTF-8", partial.display()))?;
    diesel::sql_query("VACUUM INTO ?")
        .bind::<Text, _>(target)
        .execute(conn)?;
    std::fs::rename(&partial, &path)?;
    Ok(path)
}

/// The snapshots in `names` older than `KEEP_DAYS` as of `now`. Names that
/// aren't snapshots are never returned.
fn to_prune(names: &[String], now: DateTime<Utc>) -> Vec<String> {
    names
        .iter()
        .filter(|name| taken_at(name).is_some_and(|at| now - at > Duration::days(KEEP_DAYS)))
        .cloned()
        .collect()
}

/// Delete the snapshots in `dir` that retention drops. Returns how many.
fn prune(dir: &Path, now: DateTime<Utc>) -> anyhow::Result<usize> {
    let names = snapshot_names(dir)?;
    let doomed = to_prune(&names, now);
    for name in &doomed {
        std::fs::remove_file(dir.join(name))?;
    }
    Ok(doomed.len())
}

fn snapshot_names(dir: &Path) -> anyhow::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        if let Ok(name) = entry?.file_name().into_string() {
            if taken_at(&name).is_some() {
                names.push(name);
            }
        }
    }
    Ok(names)
}

fn newest(dir: &Path) -> anyhow::Result<Option<DateTime<Utc>>> {
    Ok(snapshot_names(dir)?
        .iter()
        .filter_map(|n| taken_at(n))
        .max())
}

/// Snapshot and prune once, on a pooled connection. Only a failed snapshot
/// is an error: a failed prune is logged, and the new snapshot still counts.
async fn run_once(pool: &DbPool, dir: PathBuf) -> anyhow::Result<()> {
    db::run_blocking(pool, move |conn| {
        let now = Utc::now();
        let path = snapshot(conn, &dir, now)?;
        tracing::info!(path = %path.display(), "database snapshot written");
        if let Err(e) = prune(&dir, now) {
            tracing::error!(dir = %dir.display(), "pruning old database snapshots failed: {e:#}");
        }
        anyhow::Ok(())
    })
    .await?
}

/// Snapshot the database into `dir` once a day, starting now unless the
/// newest snapshot there is less than a day old. A failure is logged and
/// retried an hour later.
pub fn spawn_scheduler(pool: Arc<DbPool>, dir: PathBuf) {
    tokio::spawn(async move {
        let mut wait = match newest(&dir) {
            Ok(Some(at)) => (at + Duration::from_std(INTERVAL).unwrap_or_default() - Utc::now())
                .to_std()
                .unwrap_or_default(),
            _ => std::time::Duration::ZERO,
        };
        loop {
            tokio::time::sleep(wait).await;
            wait = match run_once(&pool, dir.clone()).await {
                Ok(()) => INTERVAL,
                Err(e) => {
                    tracing::error!(dir = %dir.display(), "database snapshot failed: {e:#}");
                    RETRY
                }
            };
        }
    });
}

/// `ramekin-server backup <dir>`: one snapshot of `database_url` into `dir`.
pub fn run_command(database_url: &str, dir: &Path) -> anyhow::Result<PathBuf> {
    // Opening a missing file would create an empty database and back that up.
    anyhow::ensure!(
        Path::new(database_url).is_file(),
        "no database at {database_url}"
    );
    std::fs::create_dir_all(dir)?;
    let mut conn = SqliteConnection::establish(database_url)?;
    db::setup_connection(&mut conn)?;
    snapshot(&mut conn, dir, Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
    }

    #[test]
    fn names_round_trip() {
        let when = at(2026, 10, 14, 9);
        assert_eq!(file_name(when), "ramekin-2026-10-14T09-00-00Z.sqlite");
        assert_eq!(taken_at(&file_name(when)), Some(when));
        assert_eq!(taken_at("notes.txt"), None);
        assert_eq!(taken_at(PARTIAL), None);
    }

    #[test]
    fn prunes_snapshots_older_than_a_week() {
        let now = at(2026, 10, 14, 12);
        let names: Vec<String> = (0..10)
            .map(|days| file_name(now - Duration::days(days)))
            .chain(["notes.txt".to_string()])
            .collect();
        assert_eq!(
            to_prune(&names, now),
            vec![
                file_name(now - Duration::days(8)),
                file_name(now - Duration::days(9)),
            ]
        );
    }

    #[test]
    fn nothing_to_prune_in_an_empty_directory() {
        assert!(to_prune(&[], Utc::now()).is_empty());
    }

    #[test]
    fn a_snapshot_is_a_complete_database() {
        let dir =
            std::env::temp_dir().join(format!("ramekin-backup-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("source.sqlite");
        let source = source.to_str().unwrap();
        let mut conn = db::open_migrated(source);
        diesel::sql_query("INSERT INTO data_migrations (name) VALUES ('backup-test')")
            .execute(&mut conn)
            .unwrap();

        let backups = dir.join("backups");
        std::fs::create_dir_all(&backups).unwrap();
        // What an interrupted attempt leaves behind doesn't block the next.
        std::fs::write(backups.join(PARTIAL), b"incomplete").unwrap();
        let path = snapshot(&mut conn, &backups, Utc::now()).unwrap();
        assert_eq!(snapshot_names(&backups).unwrap().len(), 1);
        assert!(!backups.join(PARTIAL).exists());

        // Restoring is starting a server on the file: it opens and migrates
        // cleanly, and holds the data.
        let mut restored = db::open_migrated(path.to_str().unwrap());
        let names: Vec<String> = crate::schema::data_migrations::table
            .select(crate::schema::data_migrations::name)
            .load(&mut restored)
            .unwrap();
        assert_eq!(names, vec!["backup-test".to_string()]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
