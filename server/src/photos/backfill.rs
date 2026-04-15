use std::sync::Arc;
use std::time::Instant;

use diesel::prelude::*;
use image::ImageReader;
use std::io::Cursor;

use crate::db::DbPool;
use crate::schema::photos;

/// How many photos to pull into memory per backfill batch. Photo blobs can be
/// up to 2 MB each, so loading every pending row at once would scale with the
/// whole table; 16 keeps peak memory bounded at ~32 MB even for large libraries.
const BACKFILL_BATCH_SIZE: i64 = 16;

/// Spawn a background task that populates `width`, `height`, and `file_size`
/// for any photo rows where those columns are NULL. Runs once on startup.
pub fn spawn_dimension_backfill(pool: Arc<DbPool>) {
    tokio::task::spawn_blocking(move || {
        if let Err(e) = run_backfill(&pool) {
            tracing::warn!("Photo dimension backfill failed: {}", e);
        }
    });
}

fn run_backfill(pool: &DbPool) -> Result<(), String> {
    let mut conn = pool.get().map_err(|e| e.to_string())?;

    // A photo needs backfill when *neither* dimensions nor file_size have been
    // written yet. We use file_size as the "attempted" flag so undecodable
    // photos can leave width/height NULL (excluding them from dim filters)
    // while still dropping out of this query.
    let total_pending: i64 = photos::table
        .filter(photos::width.is_null())
        .filter(photos::file_size.is_null())
        .filter(photos::deleted_at.is_null())
        .count()
        .get_result(&mut conn)
        .map_err(|e| e.to_string())?;

    if total_pending == 0 {
        return Ok(());
    }

    let start = Instant::now();
    tracing::info!("Backfilling dimensions for {} photos...", total_pending);

    let mut ok = 0;
    let mut failed = 0;
    loop {
        // Re-query each batch so already-processed rows drop out of the
        // pending set. This keeps memory use bounded by BACKFILL_BATCH_SIZE
        // rather than growing with the total number of pending photos.
        let batch: Vec<(uuid::Uuid, Vec<u8>)> = photos::table
            .filter(photos::width.is_null())
            .filter(photos::file_size.is_null())
            .filter(photos::deleted_at.is_null())
            .select((photos::id, photos::data))
            .limit(BACKFILL_BATCH_SIZE)
            .load(&mut conn)
            .map_err(|e| e.to_string())?;

        if batch.is_empty() {
            break;
        }

        for (id, data) in batch {
            let file_size = data.len() as i32;
            match decode_dimensions(&data) {
                Ok((w, h)) => {
                    if diesel::update(photos::table.find(id))
                        .set((
                            photos::width.eq(Some(w as i32)),
                            photos::height.eq(Some(h as i32)),
                            photos::file_size.eq(Some(file_size)),
                        ))
                        .execute(&mut conn)
                        .is_ok()
                    {
                        ok += 1;
                    } else {
                        failed += 1;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to decode photo {}: {}", id, e);
                    // Leave width/height NULL so this photo is excluded from
                    // BOTH `photo_dim:<N` and `photo_dim:>N` (the raw-SQL
                    // filters already check width/height IS NOT NULL). We
                    // still record file_size, which both makes the bytes
                    // filter work *and* marks the row as "backfill attempted"
                    // so it falls out of the pending query.
                    let _ = diesel::update(photos::table.find(id))
                        .set(photos::file_size.eq(Some(file_size)))
                        .execute(&mut conn);
                    failed += 1;
                }
            }
        }
    }

    tracing::info!(
        "Photo dimension backfill complete: {} ok, {} failed, {}ms",
        ok,
        failed,
        start.elapsed().as_millis()
    );
    Ok(())
}

fn decode_dimensions(data: &[u8]) -> Result<(u32, u32), String> {
    let reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let dims = reader.into_dimensions().map_err(|e| e.to_string())?;
    Ok(dims)
}
