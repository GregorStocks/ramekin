use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::NewPhotoThumbnail;
use crate::photos::processing::{generate_thumbnail, MAX_THUMBNAIL_SIZE, THUMBNAIL_SIZE};
use crate::schema::{photo_thumbnails, photos};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use diesel::prelude::*;
use serde::Deserialize;
use std::sync::Arc;
use utoipa::IntoParams;
use uuid::Uuid;

#[derive(Debug, Deserialize, IntoParams)]
pub struct ThumbnailParams {
    /// Desired thumbnail size in pixels (longest edge). Clamped to 1..=800. Default: 200.
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
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    let size = params
        .size
        .unwrap_or(THUMBNAIL_SIZE)
        .clamp(1, MAX_THUMBNAIL_SIZE);

    // Fast path: size=200 uses the pre-generated photos.thumbnail column
    if size == THUMBNAIL_SIZE {
        let thumbnail: Vec<u8> = match photos::table
            .filter(photos::id.eq(id))
            .filter(photos::user_id.eq(user.id))
            .filter(photos::deleted_at.is_null())
            .select(photos::thumbnail)
            .first(&mut conn)
        {
            Ok(t) => t,
            Err(diesel::result::Error::NotFound) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "Photo not found".to_string(),
                    }),
                )
                    .into_response()
            }
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to fetch photo".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        return jpeg_response(thumbnail).into_response();
    }

    // Verify photo exists and belongs to user (without loading the full blob)
    let photo_exists: bool = match photos::table
        .filter(photos::id.eq(id))
        .filter(photos::user_id.eq(user.id))
        .filter(photos::deleted_at.is_null())
        .select(diesel::dsl::count_star().gt(0))
        .first(&mut conn)
    {
        Ok(exists) => exists,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch photo".to_string(),
                }),
            )
                .into_response()
        }
    };

    if !photo_exists {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Photo not found".to_string(),
            }),
        )
            .into_response();
    }

    // Check the thumbnail cache
    let cached: Option<Vec<u8>> = photo_thumbnails::table
        .filter(photo_thumbnails::photo_id.eq(id))
        .filter(photo_thumbnails::size.eq(size as i32))
        .select(photo_thumbnails::data)
        .first(&mut conn)
        .optional()
        .unwrap_or(None);

    if let Some(data) = cached {
        return jpeg_response(data).into_response();
    }

    // Cache miss: load the full image and generate
    let full_data: Vec<u8> = match photos::table
        .filter(photos::id.eq(id))
        .select(photos::data)
        .first(&mut conn)
    {
        Ok(d) => d,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to load photo data".to_string(),
                }),
            )
                .into_response()
        }
    };

    let thumb_bytes = match generate_thumbnail(&full_data, size) {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!("Failed to generate thumbnail: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to generate thumbnail".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Cache it (race-safe: ON CONFLICT DO NOTHING)
    let _ = diesel::insert_into(photo_thumbnails::table)
        .values(&NewPhotoThumbnail {
            photo_id: id,
            size: size as i32,
            data: &thumb_bytes,
        })
        .on_conflict((photo_thumbnails::photo_id, photo_thumbnails::size))
        .do_nothing()
        .execute(&mut conn);

    jpeg_response(thumb_bytes).into_response()
}
