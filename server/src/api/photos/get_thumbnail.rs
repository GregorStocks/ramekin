use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::Photo;
use crate::schema::photos;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use diesel::prelude::*;
use std::sync::Arc;
use uuid::Uuid;

#[utoipa::path(
    get,
    path = "/api/photos/{id}/thumbnail",
    tag = "photos",
    params(
        ("id" = Uuid, Path, description = "Photo ID")
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
) -> impl IntoResponse {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Database connection failed".to_string(),
                }),
            )
                .into_response()
        }
    };

    let photo: Photo = match photos::table
        .filter(photos::id.eq(id))
        .filter(photos::user_id.eq(user.id))
        .filter(photos::deleted_at.is_null())
        .select(Photo::as_select())
        .first(&mut conn)
    {
        Ok(p) => p,
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

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/jpeg")
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .body(Body::from(photo.thumbnail))
        .unwrap()
        .into_response()
}
