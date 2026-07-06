use super::paprika::export_recipe_to_paprikarecipe;
use super::read::RecipeWithVersion;
use crate::db::DbPool;
use bytes::Bytes;
use std::io::{self, Write};
use tokio::sync::mpsc;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// `std::io::Write` that forwards writes as `Bytes` chunks to a tokio mpsc
/// channel. Lets a blocking ZIP writer stream output to an axum body without
/// buffering the whole archive in memory.
struct ChannelWriter {
    tx: mpsc::Sender<Result<Bytes, io::Error>>,
}

impl Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.tx
            .blocking_send(Ok(Bytes::copy_from_slice(buf)))
            .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Write every recipe in `recipes` as a .paprikarecipe entry to a streaming
/// ZIP. Any per-recipe failure (DB error fetching photos/tags, corrupt
/// stored data, zip metadata rejection) aborts the stream: an export is a
/// backup, and a 200 archive that silently omits a recipe is partial data
/// loss the client has no way to notice. Writer IO errors (client disconnect
/// surfacing as a broken pipe from ChannelWriter) also abort so we don't
/// keep doing expensive DB/CPU work with nowhere to send the bytes.
///
/// A fresh DB connection is checked out per recipe and dropped before the
/// (potentially slow, backpressured) zip write, so a slow client can't pin
/// a pool connection for the whole download.
pub(super) fn write_zip_stream(
    pool: &DbPool,
    user_id: Uuid,
    recipes: &[RecipeWithVersion],
    tx: mpsc::Sender<Result<Bytes, io::Error>>,
) -> io::Result<u64> {
    let writer = ChannelWriter { tx };
    // new_stream uses data descriptors and avoids seek operations, so the
    // zip bytes can be produced and forwarded to the network without ever
    // materializing the whole archive in memory.
    let mut zip = ZipWriter::new_stream(writer);
    // Store without additional compression since each .paprikarecipe is already gzipped
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    let mut total_entry_bytes: u64 = 0;
    for recipe in recipes {
        // Short-lived per-recipe connection: held during the DB fetch and
        // in-memory encoding, released before the slow zip write.
        let exported = {
            let mut conn = pool.get().map_err(|e| {
                tracing::error!(
                    recipe_id = %recipe.id,
                    title = %recipe.version.title,
                    error = %e,
                    "failed to acquire db connection during export; aborting stream"
                );
                io::Error::other(format!("db pool: {}", e))
            })?;
            export_recipe_to_paprikarecipe(&mut conn, user_id, recipe).map_err(|e| {
                tracing::error!(
                    recipe_id = %recipe.id,
                    title = %recipe.version.title,
                    error = %e,
                    "failed to export recipe; aborting stream"
                );
                io::Error::other(format!("export recipe {}: {}", recipe.id, e))
            })?
        };

        zip.start_file(&exported.filename, options).map_err(|e| {
            // IO errors here almost always mean the client has gone away;
            // anything else (zip metadata rejection) is still a recipe we
            // would otherwise silently drop from the backup.
            tracing::error!(
                recipe_id = %recipe.id,
                title = %recipe.version.title,
                error = %e,
                "failed to start zip entry; aborting stream"
            );
            match e {
                zip::result::ZipError::Io(e) => e,
                e => io::Error::other(format!("zip entry for {}: {}", recipe.id, e)),
            }
        })?;
        let entry_bytes = exported.data.len() as u64;
        zip.write_all(&exported.data)?;
        total_entry_bytes += entry_bytes;
    }

    zip.finish()
        .map_err(|e| io::Error::other(format!("finalize zip: {}", e)))?;
    Ok(total_entry_bytes)
}
