use crate::api::ErrorResponse;
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
        Err(e) => {
            tracing::warn!("Multipart read error: {}", e);
            let error_msg = if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                "File too large. Maximum size is 2MB".to_string()
            } else {
                format!("Failed to read multipart data: {}", e.body_text())
            };
            return (e.status(), Json(ErrorResponse { error: error_msg })).into_response();
        }
    };

    // Read file data
    let data = match field.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!("Field read error: {}", e);
            let error_msg = if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                "File too large. Maximum size is 2MB".to_string()
            } else {
                format!("Failed to read file data: {}", e.body_text())
            };
            return (e.status(), Json(ErrorResponse { error: error_msg })).into_response();
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

    // Process image: detect format from bytes, validate, and generate thumbnail
    let (content_type, thumbnail) = match process_image(&data) {
        Ok(result) => result,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response()
        }
    };

    // Get database connection
    let mut conn = get_conn!(pool);

    // Insert photo
    let new_photo = NewPhoto {
        user_id: user.id,
        content_type: &content_type,
        data: &data,
        thumbnail: &thumbnail,
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
