use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::{Ingredient, NewRecipeVersion};
use crate::recipes::{create_new_version, TagSource};
use crate::schema::{recipe_versions, recipes};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use ramekin_core::ai::{normalize_title as ai_normalize_title, CachingAiClient};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NormalizeTitleResponse {
    pub original_title: String,
    pub normalized_title: String,
    pub changed: bool,
    pub cached: bool,
}

/// Flatten stored JSON ingredients into a comma-separated string for LLM context.
fn format_ingredients_for_prompt(ingredients: &serde_json::Value) -> String {
    let Ok(items) = serde_json::from_value::<Vec<Ingredient>>(ingredients.clone()) else {
        return ingredients.to_string();
    };
    items
        .iter()
        .map(|i| {
            let measurement = i
                .measurements
                .first()
                .map(|m| {
                    format!(
                        "{} {}",
                        m.amount.as_deref().unwrap_or(""),
                        m.unit.as_deref().unwrap_or("")
                    )
                })
                .unwrap_or_default();
            format!("{} {}", measurement, i.item).trim().to_string()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[allow(clippy::type_complexity)]
type CurrentVersionRow = (
    Uuid,              // recipes.id
    String,            // title
    Option<String>,    // description
    serde_json::Value, // ingredients
    String,            // instructions
    Option<String>,    // source_url
    Option<String>,    // source_name
    Vec<Option<Uuid>>, // photo_ids
    Option<String>,    // servings
    Option<String>,    // prep_time
    Option<String>,    // cook_time
    Option<String>,    // total_time
    Option<i32>,       // rating
    Option<String>,    // difficulty
    Option<String>,    // nutritional_info
    Option<String>,    // notes
);

#[utoipa::path(
    post,
    path = "/api/recipes/{id}/normalize-title",
    tag = "recipes",
    params(
        ("id" = Uuid, Path, description = "Recipe ID")
    ),
    responses(
        (status = 200, description = "Title normalized and applied", body = NormalizeTitleResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Recipe not found", body = ErrorResponse),
        (status = 503, description = "AI service unavailable", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn normalize_title(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(recipe_id): Path<Uuid>,
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    let current: CurrentVersionRow = match recipes::table
        .inner_join(
            recipe_versions::table.on(recipe_versions::id
                .nullable()
                .eq(recipes::current_version_id)),
        )
        .filter(recipes::id.eq(recipe_id))
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .select((
            recipes::id,
            recipe_versions::title,
            recipe_versions::description,
            recipe_versions::ingredients,
            recipe_versions::instructions,
            recipe_versions::source_url,
            recipe_versions::source_name,
            recipe_versions::photo_ids,
            recipe_versions::servings,
            recipe_versions::prep_time,
            recipe_versions::cook_time,
            recipe_versions::total_time,
            recipe_versions::rating,
            recipe_versions::difficulty,
            recipe_versions::nutritional_info,
            recipe_versions::notes,
        ))
        .first(&mut conn)
    {
        Ok(r) => r,
        Err(diesel::NotFound) => return ApiError::not_found("Recipe not found").into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch recipe: {}", e);
            return ApiError::internal("Failed to fetch recipe").into_response();
        }
    };

    let (
        recipe_id,
        original_title,
        description,
        ingredients,
        instructions,
        source_url,
        source_name,
        photo_ids,
        servings,
        prep_time,
        cook_time,
        total_time,
        rating,
        difficulty,
        nutritional_info,
        notes,
    ) = current;

    let ai_client = match CachingAiClient::from_env() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("AI client unavailable: {}", e);
            return ApiError::service_unavailable("AI service unavailable").into_response();
        }
    };

    let ingredients_str = format_ingredients_for_prompt(&ingredients);

    let result = match ai_normalize_title(
        &ai_client,
        &original_title,
        &ingredients_str,
        &instructions,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("normalize_title call failed: {}", e);
            return ApiError::service_unavailable(format!("Title normalization failed: {}", e))
                .into_response();
        }
    };

    let new_title = result.normalized_title.trim().to_string();
    let changed = !new_title.is_empty() && new_title != original_title;

    if !changed {
        return (
            StatusCode::OK,
            Json(NormalizeTitleResponse {
                original_title: original_title.clone(),
                normalized_title: original_title,
                changed: false,
                cached: result.cached,
            }),
        )
            .into_response();
    }

    let write_result: Result<(), diesel::result::Error> = conn.transaction(|conn| {
        let new_version = NewRecipeVersion {
            recipe_id,
            title: &new_title,
            description: description.as_deref(),
            ingredients,
            instructions: &instructions,
            source_url: source_url.as_deref(),
            source_name: source_name.as_deref(),
            photo_ids: &photo_ids,
            servings: servings.as_deref(),
            prep_time: prep_time.as_deref(),
            cook_time: cook_time.as_deref(),
            total_time: total_time.as_deref(),
            rating,
            difficulty: difficulty.as_deref(),
            nutritional_info: nutritional_info.as_deref(),
            notes: notes.as_deref(),
            version_source: "normalize_title",
        };

        let old_version_id: Option<Uuid> = recipes::table
            .filter(recipes::id.eq(recipe_id))
            .select(recipes::current_version_id)
            .first(conn)?;

        let tag_source = old_version_id.map_or(TagSource::None, TagSource::CopyFrom);
        create_new_version(conn, &new_version, tag_source)?;

        Ok(())
    });

    if let Err(e) = write_result {
        tracing::error!("Failed to persist normalized title: {}", e);
        return ApiError::internal("Failed to persist normalized title").into_response();
    }

    (
        StatusCode::OK,
        Json(NormalizeTitleResponse {
            original_title,
            normalized_title: new_title,
            changed: true,
            cached: result.cached,
        }),
    )
        .into_response()
}
