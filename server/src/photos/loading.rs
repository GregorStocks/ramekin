use crate::db::DbPool;
use crate::ids::Uuid;
use crate::models::Photo;
use crate::photos::store::read_live;
use crate::schema::photos;
use base64::Engine;
use diesel::prelude::*;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PhotoImageLoadError {
    #[error("One or more photos not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(String),
    #[error("Photo file error: {0}")]
    File(String),
}

/// Load owned photos' bytes from their files and convert them to vision inputs.
pub async fn load_photo_images(
    pool: &DbPool,
    user_id: Uuid,
    photo_ids: &[Uuid],
) -> Result<Vec<ramekin_core::ai::ImageData>, PhotoImageLoadError> {
    if photo_ids.is_empty() {
        return Ok(vec![]);
    }

    let photo_ids = photo_ids.to_vec();
    let photos = crate::db::run_blocking(pool, move |conn| find_photos(conn, user_id, &photo_ids))
        .await
        .map_err(|e| PhotoImageLoadError::Database(e.to_string()))??;

    let mut images = Vec::with_capacity(photos.len());
    for (data_hash, content_type) in photos {
        let data = read_live(data_hash)
            .await
            .map_err(|e| PhotoImageLoadError::File(format!("{e:#}")))?;
        images.push(ramekin_core::ai::ImageData {
            base64: base64::engine::general_purpose::STANDARD.encode(&data),
            content_type,
        });
    }
    Ok(images)
}

/// The file hash and content type of each owned, live photo `photo_ids`
/// names, in that order.
fn find_photos(
    conn: &mut diesel::SqliteConnection,
    user_id: Uuid,
    photo_ids: &[Uuid],
) -> Result<Vec<(String, String)>, PhotoImageLoadError> {
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

    photo_ids
        .iter()
        .map(|id| {
            photos_by_id
                .get(id)
                .map(|photo| (photo.data_hash.clone(), photo.content_type.clone()))
                .ok_or(PhotoImageLoadError::NotFound)
        })
        .collect()
}
