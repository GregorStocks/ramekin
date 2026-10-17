//! Large immutable bytes live in files beside the database rather than in
//! it, so the database stays small and backing it up stays cheap: photos and
//! their thumbnails (crate::photos::store), and large step outputs such as
//! fetched pages (crate::scraping::output_store).
//!
//! A file is named by the SHA-256 of its contents, under a directory named
//! for the hash's first two hex digits: `<database>-files/ab/ab12…`. A file
//! never changes once written and is never deleted (a soft-deleted photo
//! keeps its file), so copying over whatever files a backup lacks brings it
//! fully up to date.

use crate::db::{self, DbPool};
use anyhow::Context;
use diesel::prelude::*;
use diesel::SqliteConnection;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct FileStore {
    dir: PathBuf,
}

impl FileStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The store for the database file at `database`: `<database>-files`.
    pub fn beside(database: &Path) -> Self {
        Self::new(suffixed(database, "-files"))
    }

    /// The store beside the database file `conn` is open on. A store from
    /// when it held only photos, `<database>-photos`, is renamed into place
    /// first.
    pub fn of(conn: &mut SqliteConnection) -> anyhow::Result<Self> {
        use pragmas::pragma_database_list as databases;
        let file: String = databases::table
            .filter(databases::name.eq("main"))
            .select(databases::file)
            .first(conn)
            .context("failed to look up the database file")?;
        anyhow::ensure!(
            !file.is_empty(),
            "stored files need a database file on disk"
        );
        let database = Path::new(&file);
        Self::adopting(&suffixed(database, "-photos"), Self::beside(database))
    }

    /// `store`, after renaming the directory `legacy` to it if only `legacy`
    /// exists. For where stores lived when they held only photos; delete it
    /// once prod and its backups have been through it.
    pub fn adopting(legacy: &Path, store: Self) -> anyhow::Result<Self> {
        if legacy.exists() {
            anyhow::ensure!(
                !store.dir.exists(),
                "both {} and {} exist; merge them by hand",
                legacy.display(),
                store.dir.display()
            );
            std::fs::rename(legacy, &store.dir).with_context(|| {
                format!(
                    "failed to rename {} to {}",
                    legacy.display(),
                    store.dir.display()
                )
            })?;
            if let Some(parent) = store.dir.parent() {
                sync_dir(parent)?;
            }
            tracing::info!(from = %legacy.display(), to = %store.dir.display(), "renamed the file store");
        }
        Ok(store)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where the file named `hash` is (or would be).
    pub fn path(&self, hash: &str) -> anyhow::Result<PathBuf> {
        anyhow::ensure!(
            hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
            "not a stored file name: {hash:?}"
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
        if path.exists() {
            // Another write may have just renamed it into place without yet
            // syncing the directories; the row about to name it must not
            // outlive it.
            sync_dirs(&path).with_context(|| format!("failed to sync {}", path.display()))?;
        } else {
            write_new(&path, |file| file.write_all(bytes))?;
        }
        Ok(hash)
    }

    /// The files rows in the database `conn` is open on name, deleted
    /// photos' included, that this store doesn't have. Rows whose bytes are
    /// still in the database (from before they moved out, or part way
    /// through the move) name no files.
    pub fn missing(&self, conn: &mut SqliteConnection) -> anyhow::Result<Vec<String>> {
        use crate::schema::{photo_thumbnails, photos, step_outputs};
        let mut hashes = Vec::new();
        if db::has_column(conn, "photos", "data_hash")? {
            hashes.extend(
                photos::table
                    .select(photos::data_hash)
                    .load::<String>(conn)?,
            );
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
        }
        if db::has_column(conn, "step_outputs", "output_hash")? {
            hashes.extend(
                step_outputs::table
                    .select(step_outputs::output_hash)
                    .load::<String>(conn)?,
            );
        }
        hashes.retain(|hash| !hash.is_empty());
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
    pub fn copy_missing_into(&self, to: &FileStore) -> anyhow::Result<usize> {
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

/// `path` with `suffix` added to its last component.
fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    name.into()
}

/// Create the file at `path` (`<store>/<shard>/<hash>`) through a temporary
/// name beside it, so it appears whole or not at all, and sync it and the
/// directories leading to it to disk, so the database row naming it never
/// outlives it in a crash.
fn write_new(
    path: &Path,
    write: impl FnOnce(&mut File) -> std::io::Result<()>,
) -> anyhow::Result<()> {
    let dir = path.parent().expect("a stored file is inside a directory");
    let partial = dir.join(format!(".partial-{}", uuid::Uuid::new_v4().simple()));
    let result = (|| {
        std::fs::create_dir_all(dir)?;
        let mut file = File::create(&partial)?;
        write(&mut file)?;
        file.sync_all()?;
        std::fs::rename(&partial, path)?;
        sync_dirs(path)
    })();
    if result.is_err() {
        // A temporary file, never a stored one.
        let _ = std::fs::remove_file(&partial);
    }
    result.with_context(|| format!("failed to write {}", path.display()))
}

/// Sync the directories leading to the file at `path`
/// (`<store>/<shard>/<hash>`): the shard, for the file's name, and the store
/// and its parent, for the shard's and the store's own entries in case the
/// write created them. A directory entry is only durable once its parent is
/// synced. Always all three, not just the ones this write created, since a
/// concurrent write may have created them without syncing yet.
fn sync_dirs(path: &Path) -> std::io::Result<()> {
    for dir in path.ancestors().skip(1).take(3) {
        sync_dir(dir)?;
    }
    Ok(())
}

fn sync_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    Ok(())
}

/// The server's store, found once at startup by [`init_live`].
static LIVE: std::sync::OnceLock<FileStore> = std::sync::OnceLock::new();

/// Find the store beside the database `pool` opens and make it the one
/// [`live`] returns. Call once at startup, before anything touches stored
/// files.
pub fn init_live(pool: &DbPool) -> anyhow::Result<()> {
    let mut conn = pool.get()?;
    let store = FileStore::of(&mut conn)?;
    LIVE.set(store)
        .map_err(|_| anyhow::anyhow!("the file store is already set"))
}

/// The server's file store.
pub fn live() -> &'static FileStore {
    LIVE.get().expect("files::init_live runs at startup")
}

/// Read the file named `hash` from the server's store, off the async runtime.
pub async fn read_live(hash: String) -> anyhow::Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || live().read(&hash)).await?
}

/// SQLite's table-valued pragmas, which the generated schema doesn't
/// describe.
pub(crate) mod pragmas {
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
}

/// A new, empty directory for a test.
#[cfg(test)]
pub fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ramekin-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_named_by_their_contents() {
        let dir = temp_dir("file-store-test");
        let store = FileStore::new(dir.join("files"));

        let hash = store.put(b"pixels").unwrap();
        assert_eq!(hash, hex::encode(Sha256::digest(b"pixels")));
        assert_eq!(store.read(&hash).unwrap(), b"pixels");
        assert_eq!(store.put(b"pixels").unwrap(), hash);
        assert!(store.path("../../etc/passwd").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn copying_brings_over_only_missing_files() {
        let dir = temp_dir("file-store-test");
        let from = FileStore::new(dir.join("from"));
        let to = FileStore::new(dir.join("to"));
        let shared = from.put(b"shared").unwrap();
        to.put(b"shared").unwrap();
        let new = from.put(b"new").unwrap();
        // What an interrupted write leaves behind isn't a stored file.
        let shard = from.path(&new).unwrap().parent().unwrap().to_path_buf();
        std::fs::write(shard.join(".partial-x"), b"ne").unwrap();

        assert_eq!(from.copy_missing_into(&to).unwrap(), 1);
        assert_eq!(to.read(&new).unwrap(), b"new");
        assert_eq!(to.read(&shared).unwrap(), b"shared");
        assert_eq!(from.copy_missing_into(&to).unwrap(), 0);
        assert_eq!(
            FileStore::new(dir.join("missing"))
                .copy_missing_into(&to)
                .unwrap(),
            0
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_photos_only_store_is_renamed_into_place() {
        let dir = temp_dir("file-store-test");
        let legacy = dir.join("db.sqlite-photos");
        let hash = FileStore::new(&legacy).put(b"photo").unwrap();

        let store =
            FileStore::adopting(&legacy, FileStore::beside(&dir.join("db.sqlite"))).unwrap();
        assert_eq!(store.dir(), dir.join("db.sqlite-files"));
        assert_eq!(store.read(&hash).unwrap(), b"photo");
        assert!(!legacy.exists());
        // Already renamed: nothing to do.
        FileStore::adopting(&legacy, FileStore::beside(&dir.join("db.sqlite"))).unwrap();

        // Both there: neither is touched.
        std::fs::create_dir(&legacy).unwrap();
        assert!(FileStore::adopting(&legacy, FileStore::beside(&dir.join("db.sqlite"))).is_err());
        assert!(legacy.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
