use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::NewPhoto;
use crate::schema::photos;
use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

pub const PATH: &str = "/api/photos";

const ALLOWED_CONTENT_TYPES: &[&str] = &["image/jpeg", "image/png", "image/gif", "image/webp"];
const MAX_FILE_SIZE: usize = 10 * 1024 * 1024; // 10MB

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct UploadPhotoResponse {
    pub id: Uuid,
}

#[derive(ToSchema)]
#[allow(dead_code)]
pub struct UploadPhotoRequest {
    #[schema(value_type = String, format = Binary)]
    pub file: Vec<u8>,
}

#[utoipa::path(
    post,
    path = "/api/photos",
    tag = "photos",
    request_body(content_type = "multipart/form-data", content = UploadPhotoRequest),
    responses(
        (status = 201, description = "Photo uploaded successfully", body = UploadPhotoResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn upload(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    // Get the file from multipart
    let field = match multipart.next_field().await {
        Ok(Some(field)) => field,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "No file provided".to_string(),
                }),
            )
                .into_response()
        }
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Failed to read multipart data".to_string(),
                }),
            )
                .into_response()
        }
    };

    // Validate content type
    let content_type = field
        .content_type()
        .unwrap_or("application/octet-stream")
        .to_string();

    if !ALLOWED_CONTENT_TYPES.contains(&content_type.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!(
                    "Invalid content type '{}'. Allowed: {}",
                    content_type,
                    ALLOWED_CONTENT_TYPES.join(", ")
                ),
            }),
        )
            .into_response();
    }

    // Read file data
    let data = match field.bytes().await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Failed to read file data".to_string(),
                }),
            )
                .into_response()
        }
    };

    // Check file size
    if data.len() > MAX_FILE_SIZE {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("File too large. Maximum size is {} bytes", MAX_FILE_SIZE),
            }),
        )
            .into_response();
    }

    // Get database connection
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

    // Insert photo
    let new_photo = NewPhoto {
        user_id: user.id,
        content_type: &content_type,
        data: &data,
    };

    let photo_id: Uuid = match diesel::insert_into(photos::table)
        .values(&new_photo)
        .returning(photos::id)
        .get_result(&mut conn)
    {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to save photo".to_string(),
                }),
            )
                .into_response()
        }
    };

    (
        StatusCode::CREATED,
        Json(UploadPhotoResponse { id: photo_id }),
    )
        .into_response()
}
