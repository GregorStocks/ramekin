use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::Photo;
use crate::schema::photos;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use diesel::prelude::*;
use std::sync::Arc;
use uuid::Uuid;

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
    let photo: Photo = run_db(&pool, move |conn| {
        photos::table
            .filter(photos::id.eq(id))
            .filter(photos::user_id.eq(user.id))
            .filter(photos::deleted_at.is_null())
            .select(Photo::as_select())
            .first(conn)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => ApiError::not_found("Photo not found"),
                _ => ApiError::internal("Failed to fetch photo"),
            })
    })
    .await?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, photo.content_type)
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .body(Body::from(photo.data))
        .unwrap())
}
