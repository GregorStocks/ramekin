use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::NewPhotoThumbnail;
use crate::photos::processing::{generate_thumbnail, MAX_THUMBNAIL_SIZE, THUMBNAIL_SIZE};
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
use uuid::Uuid;

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

    // Fast path: size=200 uses the pre-generated photos.thumbnail column
    if size == THUMBNAIL_SIZE {
        let thumbnail: Vec<u8> = run_db(&pool, move |conn| {
            photos::table
                .filter(photos::id.eq(id))
                .filter(photos::user_id.eq(user_id))
                .filter(photos::deleted_at.is_null())
                .select(photos::thumbnail)
                .first(conn)
                .map_err(|e| match e {
                    diesel::result::Error::NotFound => ApiError::not_found("Photo not found"),
                    _ => ApiError::internal("Failed to fetch photo"),
                })
        })
        .await?;

        return Ok(jpeg_response(thumbnail));
    }

    let thumb_bytes: Vec<u8> = run_db(&pool, move |conn| {
        // Verify photo exists and belongs to user (without loading the full blob)
        let photo_exists: bool = photos::table
            .filter(photos::id.eq(id))
            .filter(photos::user_id.eq(user_id))
            .filter(photos::deleted_at.is_null())
            .select(diesel::dsl::count_star().gt(0))
            .first(conn)
            .map_err(|_| ApiError::internal("Failed to fetch photo"))?;

        if !photo_exists {
            return Err(ApiError::not_found("Photo not found"));
        }

        // Check the thumbnail cache
        let cached: Option<Vec<u8>> = photo_thumbnails::table
            .filter(photo_thumbnails::photo_id.eq(id))
            .filter(photo_thumbnails::size.eq(size as i32))
            .select(photo_thumbnails::data)
            .first(conn)
            .optional()
            .map_err(|_| ApiError::internal("Failed to fetch thumbnail cache"))?;

        if let Some(data) = cached {
            return Ok(data);
        }

        // Cache miss: load the full image and generate
        let full_data: Vec<u8> = photos::table
            .filter(photos::id.eq(id))
            .filter(photos::user_id.eq(user_id))
            .filter(photos::deleted_at.is_null())
            .select(photos::data)
            .first(conn)
            .map_err(|_| ApiError::internal("Failed to load photo data"))?;

        let thumb_bytes = generate_thumbnail(&full_data, size).map_err(|e| {
            tracing::error!("Failed to generate thumbnail: {}", e);
            ApiError::internal("Failed to generate thumbnail")
        })?;

        // Cache it (race-safe: ON CONFLICT DO NOTHING)
        let _ = diesel::insert_into(photo_thumbnails::table)
            .values(&NewPhotoThumbnail {
                photo_id: id,
                size: size as i32,
                data: &thumb_bytes,
            })
            .on_conflict((photo_thumbnails::photo_id, photo_thumbnails::size))
            .do_nothing()
            .execute(conn);

        Ok(thumb_bytes)
    })
    .await?;

    Ok(jpeg_response(thumb_bytes))
}
