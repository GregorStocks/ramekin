use crate::api::ai::ai_client_from_env;
use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::{Ingredient, NewRecipeVersion, RecipeVersion};
use crate::recipes::{create_new_version_cas, TagSource, VersionWriteError};
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
        (status = 409, description = "Recipe was modified concurrently", body = ErrorResponse),
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
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;

    // Read recipe snapshot in its own run_db block so no DB connection is held
    // across the (potentially slow) AI call.
    let current_version: RecipeVersion = run_db(&pool, move |conn| {
        recipes::table
            .inner_join(
                recipe_versions::table.on(recipe_versions::id
                    .nullable()
                    .eq(recipes::current_version_id)),
            )
            .filter(recipes::id.eq(recipe_id))
            .filter(recipes::user_id.eq(user_id))
            .filter(recipes::deleted_at.is_null())
            .select(RecipeVersion::as_select())
            .first(conn)
            .map_err(|e| match e {
                diesel::NotFound => ApiError::not_found("Recipe not found"),
                e => {
                    tracing::error!("Failed to fetch recipe: {}", e);
                    ApiError::internal("Failed to fetch recipe")
                }
            })
    })
    .await?;

    let original_title = current_version.title.clone();

    let ai_client = ai_client_from_env()?;

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
            return Err(ApiError::service_unavailable(format!(
                "Title normalization failed: {}",
                e
            )));
        }
    };

    let new_title = result.normalized_title.trim().to_string();
    let changed = !new_title.is_empty() && new_title != original_title;

    if !changed {
        return Ok((
            StatusCode::OK,
            Json(NormalizeTitleResponse {
                original_title: original_title.clone(),
                normalized_title: original_title,
                changed: false,
                cached: result.cached,
            }),
        ));
    }

    let title_for_write = new_title.clone();
    run_db(&pool, move |conn| {
        let write_result: Result<(), VersionWriteError> = conn.transaction(|conn| {
            let new_version = NewRecipeVersion {
                title: &title_for_write,
                ..NewRecipeVersion::copy_of(&current_version, "normalize_title")
            };

            create_new_version_cas(
                conn,
                &new_version,
                Some(current_version.id),
                TagSource::CopyFrom(current_version.id),
            )?;

            Ok(())
        });

        match write_result {
            Ok(()) => Ok(()),
            Err(VersionWriteError::Stale) => Err(ApiError::conflict(
                "Recipe was modified while normalizing its title; try again",
            )),
            Err(VersionWriteError::Db(e)) => {
                tracing::error!("Failed to persist normalized title: {}", e);
                Err(ApiError::internal("Failed to persist normalized title"))
            }
        }
    })
    .await?;

    Ok((
        StatusCode::OK,
        Json(NormalizeTitleResponse {
            original_title,
            normalized_title: new_title,
            changed: true,
            cached: result.cached,
        }),
    ))
}
