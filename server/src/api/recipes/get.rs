use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::Ingredient;
use crate::raw_sql;
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

// Type alias for the query result row (all version fields plus tags via correlated subquery)
#[allow(clippy::type_complexity)]
type RecipeRow = (
    DateTime<Utc>,     // recipes.created_at
    Uuid,              // recipe_versions.id (version_id)
    String,            // title
    Option<String>,    // description
    serde_json::Value, // ingredients (JSON)
    String,            // instructions
    Option<String>,    // source_url
    Option<String>,    // source_name
    Vec<Option<Uuid>>, // photo_ids
    DateTime<Utc>,     // recipe_versions.created_at (updated_at)
    Option<String>,    // servings
    Option<String>,    // prep_time
    Option<String>,    // cook_time
    Option<String>,    // total_time
    Option<i32>,       // rating
    Option<String>,    // difficulty
    Option<String>,    // nutritional_info
    Option<String>,    // notes
    String,            // version_source
    Vec<String>,       // tags (from correlated subquery)
);

/// Common select columns for recipe queries, including tags via correlated subquery
macro_rules! recipe_select {
    () => {
        (
            recipes::created_at,
            recipe_versions::id,
            recipe_versions::title,
            recipe_versions::description,
            recipe_versions::ingredients,
            recipe_versions::instructions,
            recipe_versions::source_url,
            recipe_versions::source_name,
            recipe_versions::photo_ids,
            recipe_versions::created_at,
            recipe_versions::servings,
            recipe_versions::prep_time,
            recipe_versions::cook_time,
            recipe_versions::total_time,
            recipe_versions::rating,
            recipe_versions::difficulty,
            recipe_versions::nutritional_info,
            recipe_versions::notes,
            recipe_versions::version_source,
            raw_sql::tags_subquery(),
        )
    };
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

    // Fetch recipe with version and tags in a single query.
    // The join condition differs based on whether we're fetching a specific version or current.
    let result: Option<RecipeRow> = match params.version_id {
        Some(version_id) => {
            // Fetch specific version
            recipes::table
                .inner_join(recipe_versions::table.on(recipe_versions::recipe_id.eq(recipes::id)))
                .filter(recipes::id.eq(id))
                .filter(recipes::user_id.eq(user.id))
                .filter(recipes::deleted_at.is_null())
                .filter(recipe_versions::id.eq(version_id))
                .select(recipe_select!())
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
                .select(recipe_select!())
                .first(&mut conn)
                .optional()
                .unwrap_or(None)
        }
    };

    let row = match result {
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

    let (
        recipe_created_at,
        version_id,
        title,
        description,
        ingredients_json,
        instructions,
        source_url,
        source_name,
        photo_ids,
        updated_at,
        servings,
        prep_time,
        cook_time,
        total_time,
        rating,
        difficulty,
        nutritional_info,
        notes,
        version_source,
        tags,
    ) = row;

    let ingredients: Vec<Ingredient> = serde_json::from_value(ingredients_json).unwrap_or_default();

    let response = RecipeResponse {
        id,
        title,
        description,
        ingredients,
        instructions,
        source_url,
        source_name,
        photo_ids: photo_ids.into_iter().flatten().collect(),
        tags,
        created_at: recipe_created_at,
        updated_at,
        servings,
        prep_time,
        cook_time,
        total_time,
        rating,
        difficulty,
        nutritional_info,
        notes,
        version_id,
        version_source,
    };

    (StatusCode::OK, Json(response)).into_response()
}
