use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::{Ingredient, NewRecipeVersion, RecipeVersion};
use crate::schema::{recipe_versions, recipes};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use serde::Deserialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateRecipeRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub ingredients: Option<Vec<Ingredient>>,
    pub instructions: Option<String>,
    pub source_url: Option<String>,
    pub source_name: Option<String>,
    pub photo_ids: Option<Vec<Uuid>>,
    pub tags: Option<Vec<String>>,
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

#[utoipa::path(
    put,
    path = "/api/recipes/{id}",
    tag = "recipes",
    params(
        ("id" = Uuid, Path, description = "Recipe ID")
    ),
    request_body = UpdateRecipeRequest,
    responses(
        (status = 200, description = "Recipe updated successfully"),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Recipe not found", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_recipe(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateRecipeRequest>,
) -> impl IntoResponse {
    if let Some(ref title) = request.title {
        if title.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Title cannot be empty".to_string(),
                }),
            )
                .into_response();
        }
    }

    if let Some(ref instructions) = request.instructions {
        if instructions.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Instructions cannot be empty".to_string(),
                }),
            )
                .into_response();
        }
    }

    let mut conn = get_conn!(pool);

    // Fetch the recipe and its current version
    let recipe: (Uuid, Option<Uuid>) = match recipes::table
        .filter(recipes::id.eq(id))
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .select((recipes::id, recipes::current_version_id))
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

    let (recipe_id, current_version_id) = recipe;

    // Fetch the current version to merge with updates
    let current_version: RecipeVersion = match current_version_id {
        Some(vid) => match recipe_versions::table
            .filter(recipe_versions::id.eq(vid))
            .first(&mut conn)
        {
            Ok(v) => v,
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to fetch current version".to_string(),
                    }),
                )
                    .into_response()
            }
        },
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Recipe has no current version".to_string(),
                }),
            )
                .into_response()
        }
    };

    // Merge request with current version
    let new_title = request.title.unwrap_or(current_version.title);
    let new_description = request.description.or(current_version.description);
    let new_ingredients = match request.ingredients {
        Some(ingredients) => match serde_json::to_value(&ingredients) {
            Ok(v) => v,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Invalid ingredients format".to_string(),
                    }),
                )
                    .into_response()
            }
        },
        None => current_version.ingredients,
    };
    let new_instructions = request.instructions.unwrap_or(current_version.instructions);
    let new_source_url = request.source_url.or(current_version.source_url);
    let new_source_name = request.source_name.or(current_version.source_name);
    let new_photo_ids: Vec<Option<Uuid>> = request
        .photo_ids
        .map(|ids| ids.into_iter().map(Some).collect())
        .unwrap_or(current_version.photo_ids);
    let new_tags: Vec<Option<String>> = request
        .tags
        .map(|tags| tags.into_iter().map(Some).collect())
        .unwrap_or(current_version.tags);
    let new_servings = request.servings.or(current_version.servings);
    let new_prep_time = request.prep_time.or(current_version.prep_time);
    let new_cook_time = request.cook_time.or(current_version.cook_time);
    let new_total_time = request.total_time.or(current_version.total_time);
    let new_rating = request.rating.or(current_version.rating);
    let new_difficulty = request.difficulty.or(current_version.difficulty);
    let new_nutritional_info = request
        .nutritional_info
        .or(current_version.nutritional_info);
    let new_notes = request.notes.or(current_version.notes);

    // Create new version in a transaction
    let result: Result<(), diesel::result::Error> = conn.transaction(|conn| {
        let new_version = NewRecipeVersion {
            recipe_id,
            title: &new_title,
            description: new_description.as_deref(),
            ingredients: new_ingredients,
            instructions: &new_instructions,
            source_url: new_source_url.as_deref(),
            source_name: new_source_name.as_deref(),
            photo_ids: &new_photo_ids,
            tags: &new_tags,
            servings: new_servings.as_deref(),
            prep_time: new_prep_time.as_deref(),
            cook_time: new_cook_time.as_deref(),
            total_time: new_total_time.as_deref(),
            rating: new_rating,
            difficulty: new_difficulty.as_deref(),
            nutritional_info: new_nutritional_info.as_deref(),
            notes: new_notes.as_deref(),
            version_source: "user",
        };

        let version_id: Uuid = diesel::insert_into(recipe_versions::table)
            .values(&new_version)
            .returning(recipe_versions::id)
            .get_result(conn)?;

        // Update recipe to point to new version
        diesel::update(recipes::table.find(recipe_id))
            .set(recipes::current_version_id.eq(version_id))
            .execute(conn)?;

        Ok(())
    });

    match result {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => {
            tracing::error!("Failed to update recipe: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to update recipe".to_string(),
                }),
            )
                .into_response()
        }
    }
}
