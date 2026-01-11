use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::Ingredient;
use crate::schema::{recipe_versions, recipes};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RecipeResponse {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub ingredients: Vec<Ingredient>,
    pub instructions: String,
    pub source_url: Option<String>,
    pub source_name: Option<String>,
    pub photo_ids: Vec<Uuid>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    /// When viewing a specific version, this is the version's created_at
    pub updated_at: DateTime<Utc>,
    // Paprika-compatible fields
    pub servings: Option<String>,
    pub prep_time: Option<String>,
    pub cook_time: Option<String>,
    pub total_time: Option<String>,
    pub rating: Option<i32>,
    pub difficulty: Option<String>,
    pub nutritional_info: Option<String>,
    pub notes: Option<String>,
    /// Version metadata
    pub version_id: Uuid,
    pub version_source: String,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct GetRecipeParams {
    /// Optional version ID to fetch a specific version instead of current
    pub version_id: Option<Uuid>,
}

#[utoipa::path(
    get,
    path = "/api/recipes/{id}",
    tag = "recipes",
    params(
        ("id" = Uuid, Path, description = "Recipe ID"),
        GetRecipeParams
    ),
    responses(
        (status = 200, description = "Recipe details", body = RecipeResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Recipe not found", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_recipe(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
    Query(params): Query<GetRecipeParams>,
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    // First verify the recipe exists and belongs to the user
    let recipe: (Uuid, DateTime<Utc>, Option<Uuid>) = match recipes::table
        .filter(recipes::id.eq(id))
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .select((
            recipes::id,
            recipes::created_at,
            recipes::current_version_id,
        ))
        .first(&mut conn)
    {
        Ok(r) => r,
        Err(diesel::NotFound) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Recipe not found".to_string(),
                }),
            )
                .into_response()
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch recipe".to_string(),
                }),
            )
                .into_response()
        }
    };

    let (recipe_id, recipe_created_at, current_version_id) = recipe;

    // Determine which version to fetch
    let version_id_to_fetch = match params.version_id {
        Some(vid) => vid,
        None => match current_version_id {
            Some(vid) => vid,
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "Recipe has no versions".to_string(),
                    }),
                )
                    .into_response()
            }
        },
    };

    // Fetch the version (with verification it belongs to this recipe)
    let version: crate::models::RecipeVersion = match recipe_versions::table
        .filter(recipe_versions::id.eq(version_id_to_fetch))
        .filter(recipe_versions::recipe_id.eq(recipe_id))
        .first(&mut conn)
    {
        Ok(v) => v,
        Err(diesel::NotFound) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Version not found".to_string(),
                }),
            )
                .into_response()
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch recipe version".to_string(),
                }),
            )
                .into_response()
        }
    };

    let ingredients: Vec<Ingredient> =
        serde_json::from_value(version.ingredients).unwrap_or_default();

    let response = RecipeResponse {
        id: recipe_id,
        title: version.title,
        description: version.description,
        ingredients,
        instructions: version.instructions,
        source_url: version.source_url,
        source_name: version.source_name,
        photo_ids: version.photo_ids.into_iter().flatten().collect(),
        tags: version.tags.into_iter().flatten().collect(),
        created_at: recipe_created_at,
        updated_at: version.created_at, // Version's created_at is the "updated" time
        servings: version.servings,
        prep_time: version.prep_time,
        cook_time: version.cook_time,
        total_time: version.total_time,
        rating: version.rating,
        difficulty: version.difficulty,
        nutritional_info: version.nutritional_info,
        notes: version.notes,
        version_id: version.id,
        version_source: version.version_source,
    };

    (StatusCode::OK, Json(response)).into_response()
}
