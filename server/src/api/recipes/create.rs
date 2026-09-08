use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::NewRecipeVersion;
use crate::recipes::{create_new_version_cas, insert_recipe, TagSource, VersionWriteError};
use crate::types::RecipeContent;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateRecipeRequest {
    #[serde(flatten)]
    pub content: RecipeContent,
    pub photo_ids: Option<Vec<Uuid>>,
    /// Reviewed ingredient lines from a text draft. Parsed by the import pipeline on save.
    pub raw_ingredients: Option<String>,
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
    Json(mut request): Json<CreateRecipeRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if request.content.title.trim().is_empty() {
        return Err(ApiError::invalid_request("Title cannot be empty"));
    }

    if request.content.instructions.trim().is_empty() {
        return Err(ApiError::invalid_request("Instructions cannot be empty"));
    }

    request.content.ingredients = if let Some(lines) = &request.raw_ingredients {
        if lines.trim().is_empty() || lines.len() > 50_000 {
            return Err(ApiError::invalid_request(
                "Enter ingredient lines between 1 and 50,000 bytes.",
            ));
        }
        let mut value = serde_json::to_value(&request.content)
            .map_err(|_| ApiError::internal("Invalid recipe content"))?;
        value["ingredients"] = serde_json::json!(lines);
        value["image_urls"] = serde_json::json!([]);
        let raw = serde_json::from_value(value)
            .map_err(|_| ApiError::internal("Invalid recipe content"))?;
        let ingredients = crate::api::import::text::parse_draft_ingredients(&raw).await?;
        if ingredients.is_empty() {
            return Err(ApiError::invalid_request(
                "Add ingredient lines before saving.",
            ));
        }
        ingredients
    } else {
        crate::api::enrich::enrich_ingredients(request.content.ingredients)
            .map_err(|_| ApiError::internal("Could not process ingredient measurements"))?
    };

    let ingredients_json = match serde_json::to_value(&request.content.ingredients) {
        Ok(v) => v,
        Err(_) => return Err(ApiError::invalid_request("Invalid ingredients format")),
    };

    let photo_ids: Vec<Option<Uuid>> = request
        .photo_ids
        .unwrap_or_default()
        .into_iter()
        .map(Some)
        .collect();

    // Normalize and validate tag names before any DB work. Store the
    // trimmed form so " course:breakfast " and "course:breakfast" resolve
    // to the same user_tags row.
    let tags: Vec<String> = request
        .content
        .tags
        .into_iter()
        .map(|t| t.trim().to_string())
        .collect();
    for tag_name in &tags {
        if let Err(err) = ramekin_core::validate_tag_name(tag_name) {
            return Err(ApiError::invalid_request(err.message().to_string()));
        }
    }

    let user_id = user.id;

    let recipe_id = run_db(&pool, move |conn| {
        // Use a transaction to create recipe + version atomically
        let result: Result<Uuid, VersionWriteError> = conn.transaction(|conn| {
            let recipe_id = insert_recipe(conn, user_id)?;

            let new_version = NewRecipeVersion {
                recipe_id,
                title: &request.content.title,
                description: request.content.description.as_deref(),
                ingredients: ingredients_json,
                instructions: &request.content.instructions,
                source_url: request.content.source_url.as_deref(),
                source_name: request.content.source_name.as_deref(),
                photo_ids: &photo_ids,
                servings: request.content.servings.as_deref(),
                prep_time: request.content.prep_time.as_deref(),
                cook_time: request.content.cook_time.as_deref(),
                total_time: request.content.total_time.as_deref(),
                rating: request.content.rating,
                difficulty: request.content.difficulty.as_deref(),
                nutritional_info: request.content.nutritional_info.as_deref(),
                notes: request.content.notes.as_deref(),
                version_source: "user",
            };

            create_new_version_cas(
                conn,
                &new_version,
                None,
                TagSource::Names {
                    user_id,
                    names: &tags,
                },
            )?;

            Ok(recipe_id)
        });

        result.map_err(|e| {
            tracing::error!("Failed to create recipe: {}", e);
            ApiError::internal("Failed to create recipe")
        })
    })
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateRecipeResponse { id: recipe_id }),
    ))
}
