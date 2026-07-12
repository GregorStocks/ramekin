use super::read::fetch_current_recipe_with_version_and_tags;
use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::{Ingredient, RecipeVersion};
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

impl RecipeResponse {
    fn from_version(
        id: Uuid,
        recipe_created_at: DateTime<Utc>,
        version: RecipeVersion,
        tags: Vec<String>,
    ) -> Result<Self, serde_json::Error> {
        let ingredients = serde_json::from_value(version.ingredients.clone())?;

        Ok(Self {
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
        })
    }
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct GetRecipeParams {
    /// Optional version ID to fetch a specific version instead of current
    pub version_id: Option<Uuid>,
}

// Type alias for the query result row (version row plus tags via correlated subquery)
type RecipeRow = (
    DateTime<Utc>, // recipes.created_at
    RecipeVersion, // recipe version
    Vec<String>,   // tags (from correlated subquery)
);

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
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;

    let row = run_db(&pool, move |conn| {
        let result: Result<Option<RecipeRow>, _> = match params.version_id {
            Some(version_id) => {
                // Fetch specific version
                recipes::table
                    .inner_join(
                        recipe_versions::table.on(recipe_versions::recipe_id.eq(recipes::id)),
                    )
                    .filter(recipes::id.eq(id))
                    .filter(recipes::user_id.eq(user_id))
                    .filter(recipes::deleted_at.is_null())
                    .filter(recipe_versions::id.eq(version_id))
                    .select((
                        recipes::created_at,
                        RecipeVersion::as_select(),
                        raw_sql::tags_subquery(),
                    ))
                    .first(conn)
                    .optional()
            }
            None => match fetch_current_recipe_with_version_and_tags(conn, user_id, id) {
                Ok((recipe, tags)) => Ok(Some((recipe.created_at, recipe.version, tags))),
                Err(diesel::NotFound) => Ok(None),
                Err(e) => Err(e),
            },
        };

        result.map_err(|e| {
            tracing::error!(recipe_id = %id, error = %e, "failed to fetch recipe");
            ApiError::internal("Failed to fetch recipe")
        })
    })
    .await?;

    let Some((recipe_created_at, version, tags)) = row else {
        return Err(ApiError::not_found("Recipe not found"));
    };

    let version_id = version.id;

    let response =
        RecipeResponse::from_version(id, recipe_created_at, version, tags).map_err(|e| {
            tracing::error!(
                recipe_id = %id,
                version_id = %version_id,
                error = %e,
                "stored ingredients JSON failed to deserialize"
            );
            ApiError::internal("Recipe ingredients are corrupt")
        })?;

    Ok((StatusCode::OK, Json(response)))
}
