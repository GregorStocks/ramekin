use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::NewPhoto;
use crate::photos::processing::{process_image, MAX_FILE_SIZE};
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

/// Allow the multipart request body to carry a MAX_FILE_SIZE image plus
/// ordinary multipart headers and boundaries.
pub const MAX_UPLOAD_BODY_SIZE: usize = MAX_FILE_SIZE + 64 * 1024;

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
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 413, description = "File too large", body = ErrorResponse)
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
        Ok(None) => return ApiError::invalid_request("No file provided").into_response(),
        Err(e) => {
            tracing::warn!("Multipart read error: {}", e);
            let err = if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                ApiError::payload_too_large(format!(
                    "File too large. Maximum size is {} bytes",
                    MAX_FILE_SIZE
                ))
            } else {
                ApiError::invalid_request(format!(
                    "Failed to read multipart data: {}",
                    e.body_text()
                ))
            };
            return err.into_response();
        }
    };

    // Read file data
    let data = match field.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!("Field read error: {}", e);
            let err = if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                ApiError::payload_too_large(format!(
                    "File too large. Maximum size is {} bytes",
                    MAX_FILE_SIZE
                ))
            } else {
                ApiError::invalid_request(format!("Failed to read file data: {}", e.body_text()))
            };
            return err.into_response();
        }
    };

    // Check file size
    if data.len() > MAX_FILE_SIZE {
        return ApiError::payload_too_large(format!(
            "File too large. Maximum size is {} bytes",
            MAX_FILE_SIZE
        ))
        .into_response();
    }

    // Process image: detect format from bytes, validate, and generate thumbnail
    let processed = match process_image(&data) {
        Ok(result) => result,
        Err(e) => return ApiError::invalid_request(e).into_response(),
    };

    // Get database connection
    let mut conn = get_conn!(pool);

    // Insert photo
    let new_photo = NewPhoto {
        user_id: user.id,
        content_type: &processed.content_type,
        data: &data,
        thumbnail: &processed.thumbnail,
        width: Some(processed.width as i32),
        height: Some(processed.height as i32),
        file_size: Some(data.len() as i32),
    };

    let photo_id: Uuid = match diesel::insert_into(photos::table)
        .values(&new_photo)
        .returning(photos::id)
        .get_result(&mut conn)
    {
        Ok(id) => id,
        Err(_) => return ApiError::internal("Failed to save photo").into_response(),
    };

    (
        StatusCode::CREATED,
        Json(UploadPhotoResponse { id: photo_id }),
    )
        .into_response()
}
