use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::schema::photos;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

pub const PATH: &str = "/api/photos";

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PhotoSummary {
    pub id: Uuid,
    pub content_type: String,
    pub created_at: DateTime<Utc>,
    /// Base64-encoded JPEG thumbnail
    pub thumbnail: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ListPhotosResponse {
    pub photos: Vec<PhotoSummary>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = photos)]
struct PhotoForList {
    id: Uuid,
    content_type: String,
    created_at: DateTime<Utc>,
    thumbnail: Vec<u8>,
}

#[utoipa::path(
    get,
    path = "/api/photos",
    tag = "photos",
    responses(
        (status = 200, description = "List of user's photos", body = ListPhotosResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_photos(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
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

    let results: Vec<PhotoForList> = match photos::table
        .filter(photos::user_id.eq(user.id))
        .filter(photos::deleted_at.is_null())
        .select(PhotoForList::as_select())
        .order(photos::created_at.desc())
        .load(&mut conn)
    {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch photos".to_string(),
                }),
            )
                .into_response()
        }
    };

    use base64::{engine::general_purpose::STANDARD, Engine};

    let photos = results
        .into_iter()
        .map(|p| PhotoSummary {
            id: p.id,
            content_type: p.content_type,
            created_at: p.created_at,
            thumbnail: STANDARD.encode(&p.thumbnail),
        })
        .collect();

    (StatusCode::OK, Json(ListPhotosResponse { photos })).into_response()
}
