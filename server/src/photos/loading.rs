use crate::db::{DbConn, DbPool};
use crate::models::Photo;
use crate::schema::photos;
use base64::Engine;
use diesel::prelude::*;
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum PhotoImageLoadError {
    #[error("One or more photos not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(String),
}

/// Load owned photo bytes from the database and convert them to vision inputs.
pub async fn load_photo_images(
    pool: &DbPool,
    user_id: Uuid,
    photo_ids: &[Uuid],
) -> Result<Vec<ramekin_core::ai::ImageData>, PhotoImageLoadError> {
    if photo_ids.is_empty() {
        return Ok(vec![]);
    }

    let photo_ids = photo_ids.to_vec();
    crate::db::run_blocking(pool, move |conn| {
        load_photo_images_with_conn(conn, user_id, &photo_ids)
    })
    .await
    .map_err(|e| PhotoImageLoadError::Database(e.to_string()))?
}

/// Same as [`load_photo_images`], but uses an already checked-out connection.
/// Async callers run this inside `crate::db::run_blocking`.
fn load_photo_images_with_conn(
    conn: &mut DbConn,
    user_id: Uuid,
    photo_ids: &[Uuid],
) -> Result<Vec<ramekin_core::ai::ImageData>, PhotoImageLoadError> {
    if photo_ids.is_empty() {
        return Ok(vec![]);
    }

    let photos_list: Vec<Photo> = photos::table
        .filter(photos::id.eq_any(photo_ids))
        .filter(photos::user_id.eq(user_id))
        .filter(photos::deleted_at.is_null())
        .load::<Photo>(conn)
        .map_err(|e| PhotoImageLoadError::Database(e.to_string()))?;

    let unique_photo_ids: HashSet<Uuid> = photo_ids.iter().copied().collect();

    if photos_list.len() != unique_photo_ids.len() {
        return Err(PhotoImageLoadError::NotFound);
    }

    let photos_by_id: HashMap<Uuid, Photo> = photos_list
        .into_iter()
        .map(|photo| (photo.id, photo))
        .collect();

    let mut ordered_images = Vec::with_capacity(photo_ids.len());
    for photo_id in photo_ids {
        let photo = photos_by_id
            .get(photo_id)
            .ok_or(PhotoImageLoadError::NotFound)?;
        let base64 = base64::engine::general_purpose::STANDARD.encode(&photo.data);
        ordered_images.push(ramekin_core::ai::ImageData {
            base64,
            content_type: photo.content_type.clone(),
        });
    }

    Ok(ordered_images)
}
