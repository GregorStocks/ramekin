use crate::api::{run_db_write, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::ids::Uuid;
use crate::models::NewUserTag;
use crate::schema::user_tags;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

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
) -> Result<impl IntoResponse, ApiError> {
    let name = request.name.trim().to_string();

    if let Err(err) = ramekin_core::validate_tag_name(&name) {
        return Err(ApiError::invalid_request(err.message().to_string()));
    }

    let user_id = user.id;

    let (id, name) = run_db_write(&pool, move |conn| {
        // Check if tag already exists (including soft-deleted)
        let existing: Option<(Uuid, String, Option<DateTime<Utc>>)> = user_tags::table
            .filter(user_tags::user_id.eq(user_id))
            .filter(user_tags::name_key.eq(crate::db::case_key(&name)))
            .select((user_tags::id, user_tags::name, user_tags::deleted_at))
            .first(conn)
            .optional()
            .map_err(|e| {
                tracing::error!("Failed to look up existing tag: {}", e);
                ApiError::internal("Failed to look up existing tag")
            })?;

        if let Some((id, existing_name, deleted_at)) = existing {
            if deleted_at.is_some() {
                // Revive the soft-deleted tag
                let now = Utc::now();
                diesel::update(user_tags::table.filter(user_tags::id.eq(id)))
                    .set((
                        user_tags::deleted_at.eq(None::<DateTime<Utc>>),
                        user_tags::updated_at.eq(now),
                    ))
                    .execute(conn)
                    .map_err(|e| {
                        tracing::error!("Failed to revive tag: {}", e);
                        ApiError::internal("Failed to create tag")
                    })?;

                return Ok((id, existing_name));
            }

            return Err(ApiError::conflict("Tag already exists"));
        }

        // Insert the new tag
        diesel::insert_into(user_tags::table)
            .values(NewUserTag {
                id: Uuid::now_v7(),
                user_id,
                name: name.as_str(),
            })
            .returning((user_tags::id, user_tags::name))
            .get_result(conn)
            .map_err(|e| {
                tracing::error!("Failed to create tag: {}", e);
                ApiError::internal("Failed to create tag")
            })
    })
    .await?;

    Ok((StatusCode::CREATED, Json(CreateTagResponse { id, name })))
}
