//! Scheduled snapshots of the SQLite database into `RAMEKIN_BACKUP_DIR`,
//! and `ramekin-server backup <dir>` for one on demand. See the Backups
//! section of doc/sqlite-cutover.md.
//!
//! A snapshot is `VACUUM INTO`: a consistent, compacted, self-contained copy
//! (no WAL) taken in a read transaction, so writers carry on meanwhile. It's
//! written under a temporary name and renamed into place, so a sync tool
//! watching the directory never picks up a partial file.
//!
//! Photo files live outside the database (crate::photos::store). After each
//! snapshot, the ones the backup directory's `photos/` lacks are copied in.
//! They never change and are never deleted, there or in the live store, so
//! that copy covers every photo any snapshot names.

use crate::db::{self, DbPool};
use crate::photos::PhotoStore;
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
/// The photo store inside a backup directory, shared by its snapshots.
const PHOTOS: &str = "photos";

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

/// Copy the photo files `dir`'s store lacks into it, from the store beside
/// the database `conn` is open on. Run it after the snapshot, so every file
/// the snapshot names is already in the live store. Returns how many it
/// copied.
fn back_up_photos(conn: &mut SqliteConnection, dir: &Path) -> anyhow::Result<usize> {
    PhotoStore::of(conn)?.copy_missing_into(&PhotoStore::new(dir.join(PHOTOS)))
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

/// Delete the snapshots in `dir` that retention drops. One that can't be
/// deleted doesn't stop the rest; the failures are reported together.
fn prune(dir: &Path, now: DateTime<Utc>) -> anyhow::Result<()> {
    let failures: Vec<String> = to_prune(&snapshot_names(dir)?, now)
        .into_iter()
        .filter_map(|name| {
            let error = std::fs::remove_file(dir.join(&name)).err()?;
            Some(format!("{name}: {error}"))
        })
        .collect();
    anyhow::ensure!(failures.is_empty(), "{}", failures.join("; "));
    Ok(())
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

/// Snapshot, back up photos and prune once, on a pooled connection. Only a
/// failed snapshot or photo copy is an error: a failed prune is logged, and the new snapshot still counts.
async fn run_once(pool: &DbPool, dir: PathBuf) -> anyhow::Result<()> {
    db::run_blocking(pool, move |conn| {
        let now = Utc::now();
        let path = snapshot(conn, &dir, now)?;
        let photos = back_up_photos(conn, &dir)?;
        tracing::info!(path = %path.display(), photos, "database snapshot written");
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

/// How far a restore moves the sync clock past the snapshot's: more than
/// the stamps any database handed out since the snapshot, so every cursor
/// issued before the restore ends up below it.
const RESTORE_CLOCK_JUMP: i64 = 1 << 40;

/// `ramekin-server restore <snapshot>`: replace the database at
/// `database_url` with `snapshot`, and copy in the photo files from the
/// `photos/` beside it that the live store lacks. Run it with the server
/// stopped. The
/// current files are kept, renamed `<database>.before-restore-<time>-<tag>`
/// (with their `-wal`/`-shm`). Clients synced past the snapshot hold cursors the
/// restored clock could later reach again, which would make their syncs skip
/// the rolled-back rows; so the clock jumps far ahead and every live row is
/// restamped above it, and every client's next sync re-delivers it all.
/// Returns where the replaced database went, if there was one.
pub fn restore(database_url: &str, snapshot: &Path) -> anyhow::Result<Option<PathBuf>> {
    anyhow::ensure!(snapshot.is_file(), "no snapshot at {}", snapshot.display());
    let database = Path::new(database_url);
    // The time for people, a random tag so two restores never share a name.
    let aside = database.exists().then(|| {
        PathBuf::from(format!(
            "{database_url}.before-restore-{}-{}",
            Utc::now().format(STAMP_FORMAT),
            uuid::Uuid::new_v4()
                .simple()
                .to_string()
                .get(..8)
                .unwrap_or_default()
        ))
    });
    if let Some(aside) = &aside {
        // A database file and its -wal/-shm only make sense together, so
        // they move as a set under the new name, and only if none of the new
        // names is taken: a rename would replace what's there.
        let moves: Vec<(PathBuf, PathBuf)> = ["", "-wal", "-shm"]
            .into_iter()
            .map(|suffix| {
                (
                    PathBuf::from(format!("{database_url}{suffix}")),
                    PathBuf::from(format!("{}{suffix}", aside.display())),
                )
            })
            .filter(|(from, _)| from.exists())
            .collect();
        for (_, to) in &moves {
            anyhow::ensure!(!to.exists(), "{} already exists", to.display());
        }
        for (from, to) in &moves {
            std::fs::rename(from, to)?;
        }
    }
    std::fs::copy(snapshot, database)?;

    let mut conn = db::open_migrated(database_url);
    db::write(&mut conn, |conn| {
        use crate::schema::sync_clock;
        let restored: i64 = sync_clock::table.select(sync_clock::value).first(conn)?;
        db::restamp_live(conn, restored + RESTORE_CLOCK_JUMP)
    })?;

    let backed_up = PhotoStore::new(
        snapshot
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(PHOTOS),
    );
    let live = PhotoStore::of(&mut conn)?;
    let photos = backed_up.copy_missing_into(&live)?;
    tracing::info!(photos, dir = %live.dir().display(), "photo files restored");
    Ok(aside)
}

/// `ramekin-server backup <dir>`: one snapshot of `database_url` into `dir`,
/// and the photo files `dir` lacks.
pub fn run_command(database_url: &str, dir: &Path) -> anyhow::Result<PathBuf> {
    // Opening a missing file would create an empty database and back that up.
    anyhow::ensure!(
        Path::new(database_url).is_file(),
        "no database at {database_url}"
    );
    std::fs::create_dir_all(dir)?;
    let mut conn = SqliteConnection::establish(database_url)?;
    db::setup_connection(&mut conn)?;
    let path = snapshot(&mut conn, dir, Utc::now())?;
    back_up_photos(&mut conn, dir)?;
    Ok(path)
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
    fn one_undeletable_snapshot_does_not_stop_pruning() {
        let dir = std::env::temp_dir().join(format!("ramekin-prune-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let now = at(2026, 10, 14, 12);
        let recent = file_name(now - Duration::days(1));
        let expired = file_name(now - Duration::days(9));
        // A directory under an expired snapshot's name: remove_file fails on it.
        let stuck = file_name(now - Duration::days(10));
        std::fs::write(dir.join(&recent), b"").unwrap();
        std::fs::write(dir.join(&expired), b"").unwrap();
        std::fs::create_dir(dir.join(&stuck)).unwrap();

        let error = prune(&dir, now).unwrap_err().to_string();

        assert!(error.contains(&stuck), "{error}");
        assert!(dir.join(&recent).exists());
        assert!(!dir.join(&expired).exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[derive(QueryableByName)]
    struct Stamp {
        #[diesel(sql_type = diesel::sql_types::Text)]
        name: String,
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        change_seq: i64,
    }

    fn tags(conn: &mut SqliteConnection) -> Vec<(String, i64)> {
        diesel::sql_query("SELECT name, change_seq FROM user_tags ORDER BY name")
            .load::<Stamp>(conn)
            .unwrap()
            .into_iter()
            .map(|t| (t.name, t.change_seq))
            .collect()
    }

    fn clock(conn: &mut SqliteConnection) -> i64 {
        use crate::schema::sync_clock;
        sync_clock::table
            .select(sync_clock::value)
            .first(conn)
            .unwrap()
    }

    #[test]
    fn restore_puts_the_snapshot_back_and_moves_every_cursor_behind_it() {
        let dir =
            std::env::temp_dir().join(format!("ramekin-restore-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let live = dir.join("live.sqlite");
        let live = live.to_str().unwrap().to_string();
        let mut conn = db::open_migrated(&live);
        let add_tag = |conn: &mut SqliteConnection, name: &str| {
            diesel::sql_query("INSERT INTO user_tags (id, user_id, name) VALUES (?, 'u', ?)")
                .bind::<Text, _>(uuid::Uuid::new_v4().to_string())
                .bind::<Text, _>(name)
                .execute(conn)
                .unwrap();
        };
        diesel::sql_query("PRAGMA foreign_keys = OFF")
            .execute(&mut conn)
            .unwrap();
        add_tag(&mut conn, "before");
        let snapshot_path = snapshot(&mut conn, &dir, Utc::now()).unwrap();
        let snapshot_clock = clock(&mut conn);
        // A change after the snapshot, which a client then synced past.
        add_tag(&mut conn, "after");
        let client_cursor = clock(&mut conn) + 1;
        drop(conn);

        let aside = restore(&live, &snapshot_path).unwrap().unwrap();

        let mut restored = db::open_migrated(&live);
        let restored_clock = clock(&mut restored);
        assert_eq!(restored_clock, snapshot_clock + RESTORE_CLOCK_JUMP);
        assert!(restored_clock > client_cursor);
        // The live row comes back to every client: stamped at or after any
        // cursor issued before the restore.
        assert_eq!(
            tags(&mut restored),
            vec![("before".to_string(), restored_clock)]
        );
        // The replaced database is kept, change and all.
        let mut replaced = SqliteConnection::establish(aside.to_str().unwrap()).unwrap();
        assert_eq!(tags(&mut replaced).len(), 2);
        drop(restored);

        // A second restore right away keeps both replaced databases.
        let second = restore(&live, &snapshot_path).unwrap().unwrap();
        assert_ne!(second, aside);
        assert!(aside.exists() && second.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn photo_files_go_with_snapshots_and_come_back_on_restore() {
        let dir = std::env::temp_dir().join(format!(
            "ramekin-photo-backup-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let live = dir.join("live.sqlite");
        let live = live.to_str().unwrap();
        drop(db::open_migrated(live));
        let hash = PhotoStore::beside(Path::new(live)).put(b"photo").unwrap();

        let backups = dir.join("backups");
        let snapshot = run_command(live, &backups).unwrap();
        assert_eq!(
            PhotoStore::new(backups.join(PHOTOS)).read(&hash).unwrap(),
            b"photo"
        );

        // Restored on a machine with no photos yet.
        std::fs::create_dir_all(dir.join("elsewhere")).unwrap();
        let fresh = dir.join("elsewhere").join("fresh.sqlite");
        let fresh = fresh.to_str().unwrap();
        restore(fresh, &snapshot).unwrap();
        let mut conn = SqliteConnection::establish(fresh).unwrap();
        assert_eq!(
            PhotoStore::of(&mut conn).unwrap().read(&hash).unwrap(),
            b"photo"
        );
        std::fs::remove_dir_all(&dir).unwrap();
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
