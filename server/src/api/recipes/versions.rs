use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::schema::{recipe_versions, recipes};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

/// Version summary for listing version history
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct VersionSummary {
    pub id: Uuid,
    pub title: String,
    pub version_source: String,
    pub created_at: DateTime<Utc>,
    pub is_current: bool,
}

/// Response for version list endpoint
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct VersionListResponse {
    pub versions: Vec<VersionSummary>,
}

#[utoipa::path(
    get,
    path = "/api/recipes/{id}/versions",
    tag = "recipes",
    params(
        ("id" = Uuid, Path, description = "Recipe ID")
    ),
    responses(
        (status = 200, description = "List of recipe versions", body = VersionListResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Recipe not found", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_versions(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;

    let (current_version_id, versions) = run_db(&pool, move |conn| {
        // First verify the recipe exists and belongs to the user, get current_version_id
        let recipe: Result<(Uuid, Option<Uuid>), _> = recipes::table
            .filter(recipes::id.eq(id))
            .filter(recipes::user_id.eq(user_id))
            .filter(recipes::deleted_at.is_null())
            .select((recipes::id, recipes::current_version_id))
            .first(conn);

        let (_recipe_id, current_version_id) = match recipe {
            Ok(r) => r,
            Err(diesel::NotFound) => return Err(ApiError::not_found("Recipe not found")),
            Err(_) => return Err(ApiError::internal("Failed to fetch recipe")),
        };

        // Fetch all versions for this recipe, ordered by created_at descending (newest first)
        let versions: Vec<(Uuid, String, String, DateTime<Utc>)> = recipe_versions::table
            .filter(recipe_versions::recipe_id.eq(id))
            .order(recipe_versions::created_at.desc())
            .select((
                recipe_versions::id,
                recipe_versions::title,
                recipe_versions::version_source,
                recipe_versions::created_at,
            ))
            .load(conn)
            .map_err(|_| ApiError::internal("Failed to fetch versions"))?;

        Ok((current_version_id, versions))
    })
    .await?;

    let summaries: Vec<VersionSummary> = versions
        .into_iter()
        .map(|(vid, title, version_source, created_at)| VersionSummary {
            id: vid,
            title,
            version_source,
            created_at,
            is_current: current_version_id == Some(vid),
        })
        .collect();

    Ok((
        StatusCode::OK,
        Json(VersionListResponse {
            versions: summaries,
        }),
    ))
}
