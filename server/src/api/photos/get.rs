use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::ids::Uuid;
use crate::models::Photo;
use crate::photos::PhotoStore;
use crate::schema::photos;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use diesel::prelude::*;
use std::sync::Arc;

#[utoipa::path(
    get,
    path = "/api/photos/{id}",
    tag = "photos",
    params(
        ("id" = Uuid, Path, description = "Photo ID")
    ),
    responses(
        (status = 200, description = "Photo data", content_type = "application/octet-stream"),
        (status = 404, description = "Photo not found", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_photo(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let (path, content_type) = run_db(&pool, move |conn| {
        let photo: Photo = photos::table
            .filter(photos::id.eq(id))
            .filter(photos::user_id.eq(user.id))
            .filter(photos::deleted_at.is_null())
            .select(Photo::as_select())
            .first(conn)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => ApiError::not_found("Photo not found"),
                _ => ApiError::internal("Failed to fetch photo"),
            })?;
        let path = PhotoStore::of(conn)
            .and_then(|store| store.path(&photo.data_hash))
            .map_err(photo_file_error)?;
        Ok((path, photo.content_type))
    })
    .await?;
    let data = tokio::fs::read(&path)
        .await
        .map_err(|e| photo_file_error(anyhow::anyhow!("{}: {e}", path.display())))?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .body(Body::from(data))
        .unwrap())
}

/// A photo whose row exists but whose file can't be found or read.
pub(super) fn photo_file_error(error: anyhow::Error) -> ApiError {
    tracing::error!("failed to read photo file: {error:#}");
    ApiError::internal("Failed to read photo")
}
