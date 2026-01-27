use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::NewUserTag;
use crate::schema::user_tags;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateTagRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CreateTagResponse {
    pub id: Uuid,
    pub name: String,
}

#[utoipa::path(
    post,
    path = "/api/tags",
    tag = "tags",
    request_body = CreateTagRequest,
    responses(
        (status = 201, description = "Tag created successfully", body = CreateTagResponse),
        (status = 400, description = "Invalid request (empty name)", body = ErrorResponse),
        (status = 409, description = "Tag already exists", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_tag(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<CreateTagRequest>,
) -> impl IntoResponse {
    let name = request.name.trim();

    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Tag name cannot be empty".to_string(),
            }),
        )
            .into_response();
    }

    let mut conn = get_conn!(pool);

    // Check if tag already exists (case-insensitive due to CITEXT)
    let existing: Option<Uuid> = user_tags::table
        .filter(user_tags::user_id.eq(user.id))
        .filter(user_tags::name.eq(name))
        .select(user_tags::id)
        .first(&mut conn)
        .optional()
        .unwrap_or(None);

    if existing.is_some() {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "Tag already exists".to_string(),
            }),
        )
            .into_response();
    }

    // Insert the new tag
    let result: Result<(Uuid, String), _> = diesel::insert_into(user_tags::table)
        .values(NewUserTag {
            user_id: user.id,
            name,
        })
        .returning((user_tags::id, user_tags::name))
        .get_result(&mut conn);

    match result {
        Ok((id, name)) => {
            (StatusCode::CREATED, Json(CreateTagResponse { id, name })).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to create tag: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to create tag".to_string(),
                }),
            )
                .into_response()
        }
    }
}
