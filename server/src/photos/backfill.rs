use std::sync::Arc;
use std::time::Instant;

use diesel::prelude::*;
use image::ImageReader;
use std::io::Cursor;

use crate::db::DbPool;
use crate::schema::photos;

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

    let pending: Vec<(uuid::Uuid, Vec<u8>)> = photos::table
        .filter(photos::width.is_null())
        .filter(photos::deleted_at.is_null())
        .select((photos::id, photos::data))
        .load(&mut conn)
        .map_err(|e| e.to_string())?;

    if pending.is_empty() {
        return Ok(());
    }

    let start = Instant::now();
    tracing::info!("Backfilling dimensions for {} photos...", pending.len());

    let mut ok = 0;
    let mut failed = 0;
    for (id, data) in pending {
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
                // Still record file_size so the bytes filter works.
                let _ = diesel::update(photos::table.find(id))
                    .set(photos::file_size.eq(Some(file_size)))
                    .execute(&mut conn);
                failed += 1;
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
