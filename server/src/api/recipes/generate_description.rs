use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::{Ingredient, NewRecipeVersion, RecipeVersion};
use crate::recipes::{create_new_version_cas, VersionWriteError};
use crate::schema::{recipe_versions, recipes};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use ramekin_core::ai::{generate_description as ai_generate_description, CachingAiClient};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct GenerateDescriptionResponse {
    pub original_description: Option<String>,
    pub generated_description: String,
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

#[utoipa::path(
    post,
    path = "/api/recipes/{id}/generate-description",
    tag = "recipes",
    params(
        ("id" = Uuid, Path, description = "Recipe ID")
    ),
    responses(
        (status = 200, description = "Description generated and applied", body = GenerateDescriptionResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Recipe not found", body = ErrorResponse),
        (status = 409, description = "Recipe was modified concurrently", body = ErrorResponse),
        (status = 503, description = "AI service unavailable", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn generate_description(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(recipe_id): Path<Uuid>,
) -> impl IntoResponse {
    // Read recipe snapshot in its own scope so the DB connection is released
    // before the (potentially slow) AI call.
    let (version_id_snapshot, current_version): (Option<Uuid>, RecipeVersion) = {
        let mut conn = get_conn!(pool);
        match recipes::table
            .inner_join(
                recipe_versions::table.on(recipe_versions::id
                    .nullable()
                    .eq(recipes::current_version_id)),
            )
            .filter(recipes::id.eq(recipe_id))
            .filter(recipes::user_id.eq(user.id))
            .filter(recipes::deleted_at.is_null())
            .select((recipes::current_version_id, RecipeVersion::as_select()))
            .first(&mut conn)
        {
            Ok(r) => r,
            Err(diesel::NotFound) => {
                return ApiError::not_found("Recipe not found").into_response()
            }
            Err(e) => {
                tracing::error!("Failed to fetch recipe: {}", e);
                return ApiError::internal("Failed to fetch recipe").into_response();
            }
        }
    };

    let original_description = current_version.description.clone();

    let ai_client = match CachingAiClient::from_env() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("AI client unavailable: {}", e);
            return ApiError::service_unavailable("AI service unavailable").into_response();
        }
    };

    let ingredients_str = format_ingredients_for_prompt(&current_version.ingredients);

    let result = match ai_generate_description(
        &ai_client,
        &current_version.title,
        &ingredients_str,
        &current_version.instructions,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("generate_description call failed: {}", e);
            return ApiError::service_unavailable(format!("Description generation failed: {}", e))
                .into_response();
        }
    };

    let new_description = result.description.trim().to_string();
    let changed =
        !new_description.is_empty() && original_description.as_deref() != Some(&new_description);

    if !changed {
        return (
            StatusCode::OK,
            Json(GenerateDescriptionResponse {
                original_description: original_description.clone(),
                generated_description: new_description,
                changed: false,
                cached: result.cached,
            }),
        )
            .into_response();
    }

    let mut conn = get_conn!(pool);
    let write_result: Result<(), VersionWriteError> = conn.transaction(|conn| {
        let new_version = NewRecipeVersion {
            description: Some(&new_description),
            ..NewRecipeVersion::copy_of(&current_version, "generate_description")
        };

        // Compare-and-swap: only repoint if current_version_id hasn't changed
        // since our initial read, preventing overwrites of concurrent edits.
        create_new_version_cas(conn, &new_version, version_id_snapshot)?;

        Ok(())
    });

    match write_result {
        Ok(()) => {}
        Err(VersionWriteError::Stale) => {
            return ApiError::conflict(
                "Recipe was modified while generating description; try again",
            )
            .into_response();
        }
        Err(VersionWriteError::Db(e)) => {
            tracing::error!("Failed to persist generated description: {}", e);
            return ApiError::internal("Failed to persist generated description").into_response();
        }
    }

    (
        StatusCode::OK,
        Json(GenerateDescriptionResponse {
            original_description,
            generated_description: new_description,
            changed: true,
            cached: result.cached,
        }),
    )
        .into_response()
}
