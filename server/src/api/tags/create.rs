use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::NewUserTag;
use crate::schema::user_tags;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
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

    if let Err(err) = ramekin_core::validate_tag_name(name) {
        return ApiError::invalid_request(err.message().to_string()).into_response();
    }

    let mut conn = get_conn!(pool);

    // Check if tag already exists (including soft-deleted)
    let existing: Option<(Uuid, String, Option<DateTime<Utc>>)> = user_tags::table
        .filter(user_tags::user_id.eq(user.id))
        .filter(user_tags::name.eq(name))
        .select((user_tags::id, user_tags::name, user_tags::deleted_at))
        .first(&mut conn)
        .optional()
        .unwrap_or(None);

    if let Some((id, existing_name, deleted_at)) = existing {
        if deleted_at.is_some() {
            // Revive the soft-deleted tag
            let now = Utc::now();
            let result = diesel::update(user_tags::table.filter(user_tags::id.eq(id)))
                .set((
                    user_tags::deleted_at.eq(None::<DateTime<Utc>>),
                    user_tags::updated_at.eq(now),
                ))
                .execute(&mut conn);

            return match result {
                Ok(_) => (
                    StatusCode::CREATED,
                    Json(CreateTagResponse {
                        id,
                        name: existing_name,
                    }),
                )
                    .into_response(),
                Err(e) => {
                    tracing::error!("Failed to revive tag: {}", e);
                    ApiError::internal("Failed to create tag").into_response()
                }
            };
        }

        return ApiError::conflict("Tag already exists").into_response();
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
            ApiError::internal("Failed to create tag").into_response()
        }
    }
}
