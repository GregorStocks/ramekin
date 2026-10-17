//! Photo bytes live in the file store beside the database
//! ([`crate::files`]); a photo row names its original and thumbnail files by
//! their hashes.

use crate::db;
use crate::files::{live, FileStore};
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::SqliteConnection;

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

/// The photo tables' legacy BLOB columns, which the generated schema doesn't
/// describe: [`move_blobs_out`] empties and drops them.
mod unlisted {
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
    use unlisted::{photo_thumbnails, photos};

    if !db::has_column(conn, "photos", "data")? {
        return Ok(());
    }
    let store = FileStore::of(conn)?;
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

    #[test]
    fn legacy_blobs_move_to_files_and_their_columns_go() {
        let dir = crate::files::temp_dir("photo-move-test");
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
        let store = FileStore::beside(&database);
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
