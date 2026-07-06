use crate::api::ai::ai_client_from_env;
use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::{Ingredient, NewRecipeVersion, RecipeVersion};
use crate::recipes::{create_new_version, TagSource};
use crate::schema::{recipe_versions, recipes};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use ramekin_core::ai::normalize_title as ai_normalize_title;
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

    let current_version: RecipeVersion = match recipes::table
        .inner_join(
            recipe_versions::table.on(recipe_versions::id
                .nullable()
                .eq(recipes::current_version_id)),
        )
        .filter(recipes::id.eq(recipe_id))
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .select(RecipeVersion::as_select())
        .first(&mut conn)
    {
        Ok(r) => r,
        Err(diesel::NotFound) => return ApiError::not_found("Recipe not found").into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch recipe: {}", e);
            return ApiError::internal("Failed to fetch recipe").into_response();
        }
    };

    let original_title = current_version.title.clone();

    let ai_client = match ai_client_from_env() {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };

    let ingredients_str = format_ingredients_for_prompt(&current_version.ingredients);

    let result = match ai_normalize_title(
        &ai_client,
        &original_title,
        &ingredients_str,
        &current_version.instructions,
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
            title: &new_title,
            ..NewRecipeVersion::copy_of(&current_version, "normalize_title")
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
