use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::{Ingredient, NewRecipe};
use crate::schema::recipes;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateRecipeRequest {
    pub title: String,
    pub description: Option<String>,
    pub ingredients: Vec<Ingredient>,
    pub instructions: String,
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

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CreateRecipeResponse {
    pub id: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/recipes",
    tag = "recipes",
    request_body = CreateRecipeRequest,
    responses(
        (status = 201, description = "Recipe created successfully", body = CreateRecipeResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_recipe(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<CreateRecipeRequest>,
) -> impl IntoResponse {
    if request.title.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Title cannot be empty".to_string(),
            }),
        )
            .into_response();
    }

    if request.instructions.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Instructions cannot be empty".to_string(),
            }),
        )
            .into_response();
    }

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

    let ingredients_json = match serde_json::to_value(&request.ingredients) {
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
    };

    let photo_ids: Vec<Option<Uuid>> = request
        .photo_ids
        .unwrap_or_default()
        .into_iter()
        .map(Some)
        .collect();

    let tags: Vec<Option<String>> = request
        .tags
        .unwrap_or_default()
        .into_iter()
        .map(Some)
        .collect();

    let new_recipe = NewRecipe {
        user_id: user.id,
        title: &request.title,
        description: request.description.as_deref(),
        ingredients: ingredients_json,
        instructions: &request.instructions,
        source_url: request.source_url.as_deref(),
        source_name: request.source_name.as_deref(),
        photo_ids: &photo_ids,
        tags: &tags,
        servings: request.servings.as_deref(),
        prep_time: request.prep_time.as_deref(),
        cook_time: request.cook_time.as_deref(),
        total_time: request.total_time.as_deref(),
        rating: request.rating,
        difficulty: request.difficulty.as_deref(),
        nutritional_info: request.nutritional_info.as_deref(),
        notes: request.notes.as_deref(),
    };

    let recipe_id: Uuid = match diesel::insert_into(recipes::table)
        .values(&new_recipe)
        .returning(recipes::id)
        .get_result(&mut conn)
    {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to create recipe: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to create recipe".to_string(),
                }),
            )
                .into_response();
        }
    };

    (
        StatusCode::CREATED,
        Json(CreateRecipeResponse { id: recipe_id }),
    )
        .into_response()
}
