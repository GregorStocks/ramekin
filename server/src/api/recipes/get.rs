use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::Ingredient;
use crate::schema::{recipe_version_tags, recipe_versions, recipes, user_tags};
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

    // Build the version filter based on whether a specific version was requested
    // We join recipes with recipe_versions in a single query
    let result: Option<(DateTime<Utc>, crate::models::RecipeVersion)> = match params.version_id {
        Some(version_id) => {
            // Fetch specific version, verifying it belongs to user's recipe
            recipes::table
                .inner_join(recipe_versions::table.on(recipe_versions::recipe_id.eq(recipes::id)))
                .filter(recipes::id.eq(id))
                .filter(recipes::user_id.eq(user.id))
                .filter(recipes::deleted_at.is_null())
                .filter(recipe_versions::id.eq(version_id))
                .select((
                    recipes::created_at,
                    crate::models::RecipeVersion::as_select(),
                ))
                .first(&mut conn)
                .optional()
                .unwrap_or(None)
        }
        None => {
            // Fetch current version
            recipes::table
                .inner_join(
                    recipe_versions::table.on(recipe_versions::id
                        .nullable()
                        .eq(recipes::current_version_id)),
                )
                .filter(recipes::id.eq(id))
                .filter(recipes::user_id.eq(user.id))
                .filter(recipes::deleted_at.is_null())
                .select((
                    recipes::created_at,
                    crate::models::RecipeVersion::as_select(),
                ))
                .first(&mut conn)
                .optional()
                .unwrap_or(None)
        }
    };

    let (recipe_created_at, version) = match result {
        Some(r) => r,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Recipe not found".to_string(),
                }),
            )
                .into_response()
        }
    };

    let ingredients: Vec<Ingredient> =
        serde_json::from_value(version.ingredients.clone()).unwrap_or_default();

    // Fetch tags from junction table
    let tags: Vec<String> = recipe_version_tags::table
        .inner_join(user_tags::table)
        .filter(recipe_version_tags::recipe_version_id.eq(version.id))
        .select(user_tags::name)
        .load(&mut conn)
        .unwrap_or_default();

    let response = RecipeResponse {
        id,
        title: version.title,
        description: version.description,
        ingredients,
        instructions: version.instructions,
        source_url: version.source_url,
        source_name: version.source_name,
        photo_ids: version.photo_ids.into_iter().flatten().collect(),
        tags,
        created_at: recipe_created_at,
        updated_at: version.created_at,
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
