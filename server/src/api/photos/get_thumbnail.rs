use super::get::read_photo_file;
use crate::api::{run_db, run_db_write, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::ids::Uuid;
use crate::models::NewPhotoThumbnail;
use crate::photos::processing::{generate_thumbnail, MAX_THUMBNAIL_SIZE, THUMBNAIL_SIZE};
use crate::photos::store::live;
use crate::schema::{photo_thumbnails, photos};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
};
use diesel::prelude::*;
use serde::Deserialize;
use std::sync::Arc;
use utoipa::IntoParams;

#[derive(Debug, Deserialize, IntoParams)]
pub struct ThumbnailParams {
    /// Desired thumbnail size in pixels (longest edge). Clamped to 1..=800. Default: 200.
    #[param(minimum = 1)]
    pub size: Option<u32>,
}

fn jpeg_response(data: Vec<u8>) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/jpeg")
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .body(Body::from(data))
        .unwrap()
}

#[utoipa::path(
    get,
    path = "/api/photos/{id}/thumbnail",
    tag = "photos",
    params(
        ("id" = Uuid, Path, description = "Photo ID"),
        ThumbnailParams,
    ),
    responses(
        (status = 200, description = "Photo thumbnail data", content_type = "image/jpeg"),
        (status = 404, description = "Photo not found", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_photo_thumbnail(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
    Query(params): Query<ThumbnailParams>,
) -> Result<Response, ApiError> {
    let user_id = user.id;

    let size = params
        .size
        .unwrap_or(THUMBNAIL_SIZE)
        .clamp(1, MAX_THUMBNAIL_SIZE);

    // A size-200 thumbnail was made with the photo. Any other size is made
    // on first request and kept in photo_thumbnails.
    let (data_hash, stored) = run_db(&pool, move |conn| {
        let (data_hash, thumbnail_hash): (String, String) = photos::table
            .filter(photos::id.eq(id))
            .filter(photos::user_id.eq(user_id))
            .filter(photos::deleted_at.is_null())
            .select((photos::data_hash, photos::thumbnail_hash))
            .first(conn)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => ApiError::not_found("Photo not found"),
                _ => ApiError::internal("Failed to fetch photo"),
            })?;
        if size == THUMBNAIL_SIZE {
            return Ok((data_hash, Some(thumbnail_hash)));
        }
        let cached: Option<String> = photo_thumbnails::table
            .filter(photo_thumbnails::photo_id.eq(id))
            .filter(photo_thumbnails::size.eq(size as i32))
            .select(photo_thumbnails::data_hash)
            .first(conn)
            .optional()
            .map_err(|_| ApiError::internal("Failed to fetch thumbnail cache"))?;
        Ok((data_hash, cached))
    })
    .await?;
    if let Some(hash) = stored {
        return Ok(jpeg_response(read_photo_file(hash).await?));
    }

    // Cache miss: generate from the full image.
    let full_data = read_photo_file(data_hash).await?;
    let (thumb_bytes, hash) = tokio::task::spawn_blocking(move || {
        let thumb_bytes = generate_thumbnail(&full_data, size).map_err(|e| {
            tracing::error!("Failed to generate thumbnail: {}", e);
            ApiError::internal("Failed to generate thumbnail")
        })?;
        let hash = live().put(&thumb_bytes).map_err(|e| {
            tracing::error!("Failed to store thumbnail: {e:#}");
            ApiError::internal("Failed to store thumbnail")
        })?;
        Ok::<_, ApiError>((thumb_bytes, hash))
    })
    .await
    .expect("thumbnail task panicked")?;

    // Cache it (race-safe: ON CONFLICT DO NOTHING)
    let _ = run_db_write(&pool, move |conn| {
        diesel::insert_into(photo_thumbnails::table)
            .values(&NewPhotoThumbnail {
                id: Uuid::now_v7(),
                photo_id: id,
                size: size as i32,
                data_hash: &hash,
            })
            .on_conflict((photo_thumbnails::photo_id, photo_thumbnails::size))
            .do_nothing()
            .execute(conn)
            .map_err(ApiError::from)
    })
    .await;

    Ok(jpeg_response(thumb_bytes))
}
