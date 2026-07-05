use super::read::fetch_current_recipe_with_version_and_tags;
use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::{Ingredient, NewRecipeVersion, RecipeVersion};
use crate::recipes::{create_new_version, TagSource};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use serde::Deserialize;
use serde_with::rust::double_option;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateRecipeRequest {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<String>)]
    pub description: Option<Option<String>>,
    pub ingredients: Option<Vec<Ingredient>>,
    pub instructions: Option<String>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<String>)]
    pub source_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<String>)]
    pub source_name: Option<Option<String>>,
    pub photo_ids: Option<Vec<Uuid>>,
    pub tags: Option<Vec<String>>,
    // Paprika-compatible fields
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<String>)]
    pub servings: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<String>)]
    pub prep_time: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<String>)]
    pub cook_time: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<String>)]
    pub total_time: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<i32>)]
    pub rating: Option<Option<i32>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<String>)]
    pub difficulty: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<String>)]
    pub nutritional_info: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    #[schema(value_type = Option<String>)]
    pub notes: Option<Option<String>>,
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
            return ApiError::invalid_request("Title cannot be empty").into_response();
        }
    }

    if let Some(ref instructions) = request.instructions {
        if instructions.trim().is_empty() {
            return ApiError::invalid_request("Instructions cannot be empty").into_response();
        }
    }

    let mut conn = get_conn!(pool);

    let (current_version, cur_tags): (RecipeVersion, Vec<String>) =
        match fetch_current_recipe_with_version_and_tags(&mut conn, user.id, id) {
            Ok((recipe, tags)) => (recipe.version, tags),
            Err(diesel::NotFound) => {
                return ApiError::not_found("Recipe not found").into_response()
            }
            Err(_) => return ApiError::internal("Failed to fetch recipe").into_response(),
        };

    // Merge request with current version
    let new_title = request
        .title
        .unwrap_or_else(|| current_version.title.clone());
    let new_description = request
        .description
        .unwrap_or_else(|| current_version.description.clone());
    let new_ingredients = match request.ingredients {
        Some(ingredients) => match serde_json::to_value(&ingredients) {
            Ok(v) => v,
            Err(_) => {
                return ApiError::invalid_request("Invalid ingredients format").into_response()
            }
        },
        None => current_version.ingredients.clone(),
    };
    let new_instructions = request
        .instructions
        .unwrap_or_else(|| current_version.instructions.clone());
    let new_source_url = request
        .source_url
        .unwrap_or_else(|| current_version.source_url.clone());
    let new_source_name = request
        .source_name
        .unwrap_or_else(|| current_version.source_name.clone());
    let new_photo_ids: Vec<Option<Uuid>> = request
        .photo_ids
        .map(|ids| ids.into_iter().map(Some).collect())
        .unwrap_or_else(|| current_version.photo_ids.clone());
    let new_tags: Vec<String> = match request.tags {
        Some(tags) => {
            // Normalize and validate before any DB work so the trimmed
            // form is what lands in user_tags.
            let tags: Vec<String> = tags.into_iter().map(|t| t.trim().to_string()).collect();
            for tag_name in &tags {
                if let Err(err) = ramekin_core::validate_tag_name(tag_name) {
                    return ApiError::invalid_request(err.message().to_string()).into_response();
                }
            }
            tags
        }
        None => cur_tags,
    };
    let new_servings = request
        .servings
        .unwrap_or_else(|| current_version.servings.clone());
    let new_prep_time = request
        .prep_time
        .unwrap_or_else(|| current_version.prep_time.clone());
    let new_cook_time = request
        .cook_time
        .unwrap_or_else(|| current_version.cook_time.clone());
    let new_total_time = request
        .total_time
        .unwrap_or_else(|| current_version.total_time.clone());
    let new_rating = request.rating.unwrap_or(current_version.rating);
    let new_difficulty = request
        .difficulty
        .unwrap_or_else(|| current_version.difficulty.clone());
    let new_nutritional_info = request
        .nutritional_info
        .unwrap_or_else(|| current_version.nutritional_info.clone());
    let new_notes = request
        .notes
        .unwrap_or_else(|| current_version.notes.clone());

    // Create new version in a transaction
    let result: Result<(), diesel::result::Error> = conn.transaction(|conn| {
        let new_version = NewRecipeVersion {
            title: &new_title,
            description: new_description.as_deref(),
            ingredients: new_ingredients,
            instructions: &new_instructions,
            source_url: new_source_url.as_deref(),
            source_name: new_source_name.as_deref(),
            photo_ids: &new_photo_ids,
            servings: new_servings.as_deref(),
            prep_time: new_prep_time.as_deref(),
            cook_time: new_cook_time.as_deref(),
            total_time: new_total_time.as_deref(),
            rating: new_rating,
            difficulty: new_difficulty.as_deref(),
            nutritional_info: new_nutritional_info.as_deref(),
            notes: new_notes.as_deref(),
            ..NewRecipeVersion::copy_of(&current_version, "user")
        };

        create_new_version(
            conn,
            &new_version,
            TagSource::Names {
                user_id: user.id,
                names: &new_tags,
            },
        )?;

        Ok(())
    });

    match result {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => {
            tracing::error!("Failed to update recipe: {}", e);
            ApiError::internal("Failed to update recipe").into_response()
        }
    }
}
