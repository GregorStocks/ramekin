use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::Ingredient;
use crate::schema::recipes;
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
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = recipes)]
struct RecipeFull {
    id: Uuid,
    title: String,
    description: Option<String>,
    ingredients: serde_json::Value,
    instructions: String,
    source_url: Option<String>,
    source_name: Option<String>,
    photo_ids: Vec<Option<Uuid>>,
    tags: Vec<Option<String>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    // Paprika-compatible fields
    servings: Option<String>,
    prep_time: Option<String>,
    cook_time: Option<String>,
    total_time: Option<String>,
    rating: Option<i32>,
    difficulty: Option<String>,
    nutritional_info: Option<String>,
    notes: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/recipes/{id}",
    tag = "recipes",
    params(
        ("id" = Uuid, Path, description = "Recipe ID")
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

    let recipe: RecipeFull = match recipes::table
        .filter(recipes::id.eq(id))
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .select(RecipeFull::as_select())
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

    let ingredients: Vec<Ingredient> =
        serde_json::from_value(recipe.ingredients).unwrap_or_default();

    let response = RecipeResponse {
        id: recipe.id,
        title: recipe.title,
        description: recipe.description,
        ingredients,
        instructions: recipe.instructions,
        source_url: recipe.source_url,
        source_name: recipe.source_name,
        photo_ids: recipe.photo_ids.into_iter().flatten().collect(),
        tags: recipe.tags.into_iter().flatten().collect(),
        created_at: recipe.created_at,
        updated_at: recipe.updated_at,
        servings: recipe.servings,
        prep_time: recipe.prep_time,
        cook_time: recipe.cook_time,
        total_time: recipe.total_time,
        rating: recipe.rating,
        difficulty: recipe.difficulty,
        nutritional_info: recipe.nutritional_info,
        notes: recipe.notes,
    };

    (StatusCode::OK, Json(response)).into_response()
}
