use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::Ingredient;
use crate::schema::recipes;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
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

#[derive(AsChangeset)]
#[diesel(table_name = recipes)]
struct RecipeUpdate {
    title: Option<String>,
    description: Option<Option<String>>,
    ingredients: Option<serde_json::Value>,
    instructions: Option<String>,
    source_url: Option<Option<String>>,
    source_name: Option<Option<String>>,
    photo_ids: Option<Vec<Option<Uuid>>>,
    tags: Option<Vec<Option<String>>>,
    updated_at: chrono::DateTime<Utc>,
    // Paprika-compatible fields
    servings: Option<Option<String>>,
    prep_time: Option<Option<String>>,
    cook_time: Option<Option<String>>,
    total_time: Option<Option<String>>,
    rating: Option<Option<i32>>,
    difficulty: Option<Option<String>>,
    nutritional_info: Option<Option<String>>,
    notes: Option<Option<String>>,
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

    // Check recipe exists and belongs to user
    let exists = match recipes::table
        .filter(recipes::id.eq(id))
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .count()
        .get_result::<i64>(&mut conn)
    {
        Ok(count) => count > 0,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to check recipe".to_string(),
                }),
            )
                .into_response()
        }
    };

    if !exists {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Recipe not found".to_string(),
            }),
        )
            .into_response();
    }

    let ingredients_json = match &request.ingredients {
        Some(ingredients) => match serde_json::to_value(ingredients) {
            Ok(v) => Some(v),
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
        None => None,
    };

    let update = RecipeUpdate {
        title: request.title,
        description: request.description.map(Some),
        ingredients: ingredients_json,
        instructions: request.instructions,
        source_url: request.source_url.map(Some),
        source_name: request.source_name.map(Some),
        photo_ids: request
            .photo_ids
            .map(|ids| ids.into_iter().map(Some).collect()),
        tags: request
            .tags
            .map(|tags| tags.into_iter().map(Some).collect()),
        updated_at: Utc::now(),
        servings: request.servings.map(Some),
        prep_time: request.prep_time.map(Some),
        cook_time: request.cook_time.map(Some),
        total_time: request.total_time.map(Some),
        rating: request.rating.map(Some),
        difficulty: request.difficulty.map(Some),
        nutritional_info: request.nutritional_info.map(Some),
        notes: request.notes.map(Some),
    };

    match diesel::update(recipes::table.filter(recipes::id.eq(id)))
        .set(&update)
        .execute(&mut conn)
    {
        Ok(_) => StatusCode::OK.into_response(),
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
