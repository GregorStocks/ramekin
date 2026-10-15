//! Photo bytes live in files beside the database rather than in it, so the
//! database stays small and backing it up stays cheap.
//!
//! A file is named by the SHA-256 of its contents, under a directory named
//! for the hash's first two hex digits: `<database>-photos/ab/ab12…`. A file
//! never changes once written and is never deleted (a soft-deleted photo
//! keeps its file), so copying over whatever files a backup lacks brings it
//! fully up to date.

use crate::db::{self, DbPool};
use anyhow::Context;
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::SqliteConnection;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct PhotoStore {
    dir: PathBuf,
}

impl PhotoStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The store for the database file at `database`: `<database>-photos`.
    pub fn beside(database: &Path) -> Self {
        let mut dir = database.as_os_str().to_owned();
        dir.push("-photos");
        Self::new(dir)
    }

    /// The store beside the database file `conn` is open on.
    pub fn of(conn: &mut SqliteConnection) -> anyhow::Result<Self> {
        use unlisted::pragma_database_list as databases;
        let file: String = databases::table
            .filter(databases::name.eq("main"))
            .select(databases::file)
            .first(conn)
            .context("failed to look up the database file")?;
        anyhow::ensure!(!file.is_empty(), "photos need a database file on disk");
        Ok(Self::beside(Path::new(&file)))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where the file named `hash` is (or would be).
    pub fn path(&self, hash: &str) -> anyhow::Result<PathBuf> {
        anyhow::ensure!(
            hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
            "not a photo file name: {hash:?}"
        );
        let shard = hash.get(..2).unwrap_or_default();
        Ok(self.dir.join(shard).join(hash))
    }

    pub fn read(&self, hash: &str) -> anyhow::Result<Vec<u8>> {
        let path = self.path(hash)?;
        std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))
    }

    /// Store `bytes` and return the hash that names them. Storing bytes that
    /// are already stored writes nothing.
    pub fn put(&self, bytes: &[u8]) -> anyhow::Result<String> {
        let hash = hex::encode(Sha256::digest(bytes));
        let path = self.path(&hash)?;
        if !path.exists() {
            write_new(&path, |file| file.write_all(bytes))?;
        }
        Ok(hash)
    }

    /// The files rows in the database `conn` is open on name, deleted
    /// photos' included, that this store doesn't have.
    pub fn missing(&self, conn: &mut SqliteConnection) -> anyhow::Result<Vec<String>> {
        use crate::schema::{photo_thumbnails, photos};
        let mut hashes: Vec<String> = photos::table.select(photos::data_hash).load(conn)?;
        hashes.extend(
            photos::table
                .select(photos::thumbnail_hash)
                .load::<String>(conn)?,
        );
        hashes.extend(
            photo_thumbnails::table
                .select(photo_thumbnails::data_hash)
                .load::<String>(conn)?,
        );
        hashes.sort_unstable();
        hashes.dedup();
        let mut missing = Vec::new();
        for hash in hashes {
            if !self.path(&hash)?.exists() {
                missing.push(hash);
            }
        }
        Ok(missing)
    }

    /// Copy every file this store has and `to` lacks into `to`. Returns how
    /// many were copied.
    pub fn copy_missing_into(&self, to: &PhotoStore) -> anyhow::Result<usize> {
        if !self.dir.is_dir() {
            return Ok(0);
        }
        let mut copied = 0;
        for shard in std::fs::read_dir(&self.dir)? {
            let shard = shard?;
            if !shard.file_type()?.is_dir() {
                continue;
            }
            for entry in std::fs::read_dir(shard.path())? {
                let entry = entry?;
                // Anything not named by a hash is a write still in progress.
                let Some(Ok(target)) = entry.file_name().to_str().map(|hash| to.path(hash)) else {
                    continue;
                };
                if target.exists() {
                    continue;
                }
                write_new(&target, |file| {
                    std::io::copy(&mut File::open(entry.path())?, file).map(|_| ())
                })?;
                copied += 1;
            }
        }
        Ok(copied)
    }
}

/// Create the file at `path` through a temporary name beside it, so it
/// appears whole or not at all, and sync it to disk, so the database row
/// naming it never outlives it in a crash.
fn write_new(
    path: &Path,
    write: impl FnOnce(&mut File) -> std::io::Result<()>,
) -> anyhow::Result<()> {
    let dir = path.parent().expect("a photo file is inside a directory");
    let partial = dir.join(format!(".partial-{}", uuid::Uuid::new_v4().simple()));
    let result = (|| {
        std::fs::create_dir_all(dir)?;
        let mut file = File::create(&partial)?;
        write(&mut file)?;
        file.sync_all()?;
        std::fs::rename(&partial, path)?;
        #[cfg(unix)]
        File::open(dir)?.sync_all()?;
        std::io::Result::Ok(())
    })();
    if result.is_err() {
        // A temporary file, never a stored photo.
        let _ = std::fs::remove_file(&partial);
    }
    result.with_context(|| format!("failed to write {}", path.display()))
}

/// The server's store, found once at startup by [`init_live`].
static LIVE: std::sync::OnceLock<PhotoStore> = std::sync::OnceLock::new();

/// Find the store beside the database `pool` opens and make it the one
/// [`live`] returns. Call once at startup, before anything touches photos.
pub fn init_live(pool: &DbPool) -> anyhow::Result<()> {
    let mut conn = pool.get()?;
    let store = PhotoStore::of(&mut conn)?;
    LIVE.set(store)
        .map_err(|_| anyhow::anyhow!("the photo store is already set"))
}

/// The server's photo store.
pub fn live() -> &'static PhotoStore {
    LIVE.get()
        .expect("photos::store::init_live runs at startup")
}

/// Read the file named `hash` from the server's store, off the async runtime.
pub async fn read_live(hash: String) -> anyhow::Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || live().read(&hash)).await?
}

/// The files of a new photo.
pub struct StoredPhoto {
    pub data_hash: String,
    pub thumbnail_hash: String,
}

/// Write a new photo's bytes and thumbnail to the server's store. Call it
/// before inserting the row that names them, and outside any write
/// transaction.
pub async fn store_photo<D>(data: D, thumbnail: Vec<u8>) -> anyhow::Result<StoredPhoto>
where
    D: AsRef<[u8]> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let store = live();
        Ok(StoredPhoto {
            data_hash: store.put(data.as_ref())?,
            thumbnail_hash: store.put(&thumbnail)?,
        })
    })
    .await?
}

/// Tables the generated schema doesn't describe: SQLite's table-valued
/// pragmas, and the photo tables' legacy BLOB columns, which
/// [`move_blobs_out`] empties and drops.
mod unlisted {
    diesel::table! {
        pragma_database_list (name) {
            name -> Text,
            file -> Text,
        }
    }

    diesel::table! {
        /// `arg`, a hidden column, names the table described.
        pragma_table_info (name) {
            name -> Text,
            arg -> Text,
        }
    }

    diesel::table! {
        photos (id) {
            id -> Text,
            data -> Binary,
            thumbnail -> Binary,
            data_hash -> Text,
            thumbnail_hash -> Text,
        }
    }

    diesel::table! {
        photo_thumbnails (id) {
            id -> Text,
            data -> Binary,
            data_hash -> Text,
        }
    }
}

/// Rows per batch when moving legacy BLOBs out. A batch's bytes are all in
/// memory at once (an original can be 10 MB), and its write transaction
/// holds up every other writer.
const MOVE_BATCH: i64 = 16;

/// Move photo bytes still in the database's legacy BLOB columns
/// (`photos.data`, `photos.thumbnail`, `photo_thumbnails.data`) out to
/// files, then drop those columns and compact the database. Each batch is
/// written to files first, then recorded and cleared in its own short
/// transaction, so an interrupted run resumes where it stopped. Does nothing
/// once the columns are gone.
pub fn move_blobs_out(conn: &mut SqliteConnection) -> anyhow::Result<()> {
    use unlisted::{photo_thumbnails, photos, pragma_table_info as columns};

    let legacy: i64 = columns::table
        .filter(columns::arg.eq("photos"))
        .filter(columns::name.eq("data"))
        .count()
        .get_result(conn)?;
    if legacy == 0 {
        return Ok(());
    }
    let store = PhotoStore::of(conn)?;
    let empty: &[u8] = &[];

    let mut moved_photos = 0;
    loop {
        let batch: Vec<(String, Vec<u8>, Vec<u8>)> = photos::table
            .filter(photos::data_hash.eq(""))
            .select((photos::id, photos::data, photos::thumbnail))
            .limit(MOVE_BATCH)
            .load(conn)?;
        if batch.is_empty() {
            break;
        }
        let stored = batch
            .iter()
            .map(|(id, data, thumbnail)| Ok((id, store.put(data)?, store.put(thumbnail)?)))
            .collect::<anyhow::Result<Vec<_>>>()?;
        db::write(conn, |conn| {
            for (id, data_hash, thumbnail_hash) in &stored {
                diesel::update(photos::table.find(id))
                    .set((
                        photos::data_hash.eq(data_hash),
                        photos::thumbnail_hash.eq(thumbnail_hash),
                        photos::data.eq(empty),
                        photos::thumbnail.eq(empty),
                    ))
                    .execute(conn)?;
            }
            QueryResult::Ok(())
        })?;
        moved_photos += batch.len();
    }

    let mut moved_thumbnails = 0;
    loop {
        let batch: Vec<(String, Vec<u8>)> = photo_thumbnails::table
            .filter(photo_thumbnails::data_hash.eq(""))
            .select((photo_thumbnails::id, photo_thumbnails::data))
            .limit(MOVE_BATCH)
            .load(conn)?;
        if batch.is_empty() {
            break;
        }
        let stored = batch
            .iter()
            .map(|(id, data)| Ok((id, store.put(data)?)))
            .collect::<anyhow::Result<Vec<_>>>()?;
        db::write(conn, |conn| {
            for (id, data_hash) in &stored {
                diesel::update(photo_thumbnails::table.find(id))
                    .set((
                        photo_thumbnails::data_hash.eq(data_hash),
                        photo_thumbnails::data.eq(empty),
                    ))
                    .execute(conn)?;
            }
            QueryResult::Ok(())
        })?;
        moved_thumbnails += batch.len();
    }

    // Every column is empty now. VACUUM hands the space the bytes took back
    // to the file system, then dropping the columns rewrites little. In that
    // order, so the columns still mark the move unfinished, and the next
    // startup retries, if VACUUM fails or is interrupted.
    conn.batch_execute("VACUUM;")?;
    db::write(conn, |conn| {
        conn.batch_execute(
            "ALTER TABLE photos DROP COLUMN data; \
             ALTER TABLE photos DROP COLUMN thumbnail; \
             ALTER TABLE photo_thumbnails DROP COLUMN data;",
        )
    })?;
    conn.batch_execute("PRAGMA wal_checkpoint(TRUNCATE);")?;
    tracing::info!(
        photos = moved_photos,
        thumbnails = moved_thumbnails,
        dir = %store.dir().display(),
        "moved photo bytes out of the database"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::sql_types::{Binary, Nullable, Text};
    use diesel_migrations::MigrationHarness;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ramekin-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn files_are_named_by_their_contents() {
        let dir = temp_dir("photo-store-test");
        let store = PhotoStore::new(dir.join("photos"));

        let hash = store.put(b"pixels").unwrap();
        assert_eq!(hash, hex::encode(Sha256::digest(b"pixels")));
        assert_eq!(store.read(&hash).unwrap(), b"pixels");
        assert_eq!(store.put(b"pixels").unwrap(), hash);
        assert!(store.path("../../etc/passwd").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn copying_brings_over_only_missing_files() {
        let dir = temp_dir("photo-store-test");
        let from = PhotoStore::new(dir.join("from"));
        let to = PhotoStore::new(dir.join("to"));
        let shared = from.put(b"shared").unwrap();
        to.put(b"shared").unwrap();
        let new = from.put(b"new").unwrap();
        // What an interrupted write leaves behind isn't a photo.
        let shard = from.path(&new).unwrap().parent().unwrap().to_path_buf();
        std::fs::write(shard.join(".partial-x"), b"ne").unwrap();

        assert_eq!(from.copy_missing_into(&to).unwrap(), 1);
        assert_eq!(to.read(&new).unwrap(), b"new");
        assert_eq!(to.read(&shared).unwrap(), b"shared");
        assert_eq!(from.copy_missing_into(&to).unwrap(), 0);
        assert_eq!(
            PhotoStore::new(dir.join("missing"))
                .copy_missing_into(&to)
                .unwrap(),
            0
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn legacy_blobs_move_to_files_and_their_columns_go() {
        let dir = temp_dir("photo-move-test");
        let database = dir.join("db.sqlite");
        let url = database.to_str().unwrap();
        let mut conn = SqliteConnection::establish(url).unwrap();
        db::setup_connection(&mut conn).unwrap();
        // A database from before photo files: the baseline schema alone.
        conn.run_next_migration(db::MIGRATIONS).unwrap();
        diesel::sql_query("INSERT INTO users (id, username, password_hash) VALUES ('u', 'u', '')")
            .execute(&mut conn)
            .unwrap();
        // More than a batch, a soft-deleted photo among them.
        for i in 0..MOVE_BATCH + 3 {
            diesel::sql_query(
                "INSERT INTO photos (id, user_id, content_type, data, thumbnail, deleted_at) \
                 VALUES (?, 'u', 'image/png', ?, ?, ?)",
            )
            .bind::<Text, _>(format!("p{i}"))
            .bind::<Binary, _>(format!("photo {i}").into_bytes())
            .bind::<Binary, _>(format!("thumb {i}").into_bytes())
            .bind::<Nullable<Text>, _>((i == 0).then_some("2026-10-14"))
            .execute(&mut conn)
            .unwrap();
        }
        diesel::sql_query(
            "INSERT INTO photo_thumbnails (id, photo_id, size, data) VALUES ('t', 'p1', 400, X'01')",
        )
        .execute(&mut conn)
        .unwrap();
        drop(conn);

        // Startup: the remaining migrations, then the move.
        let mut conn = db::open_migrated(url);

        use unlisted::{photo_thumbnails, photos};
        let store = PhotoStore::beside(&database);
        for i in [0, MOVE_BATCH + 2] {
            let (data_hash, thumbnail_hash): (String, String) = photos::table
                .find(format!("p{i}"))
                .select((photos::data_hash, photos::thumbnail_hash))
                .first(&mut conn)
                .unwrap();
            assert_eq!(
                store.read(&data_hash).unwrap(),
                format!("photo {i}").into_bytes()
            );
            assert_eq!(
                store.read(&thumbnail_hash).unwrap(),
                format!("thumb {i}").into_bytes()
            );
        }
        let thumbnail: String = photo_thumbnails::table
            .select(photo_thumbnails::data_hash)
            .first(&mut conn)
            .unwrap();
        assert_eq!(store.read(&thumbnail).unwrap(), vec![1]);
        assert!(photos::table
            .select(photos::data)
            .first::<Vec<u8>>(&mut conn)
            .is_err());

        // Run again, as every later startup does: nothing to do.
        move_blobs_out(&mut conn).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
