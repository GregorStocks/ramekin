use crate::api::ai::ai_client_from_env;
use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
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
use ramekin_core::ai::generate_description as ai_generate_description;
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
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;

    // Read recipe snapshot in its own run_db block so no DB connection is held
    // across the (potentially slow) AI call.
    let (version_id_snapshot, current_version): (Option<Uuid>, RecipeVersion) =
        run_db(&pool, move |conn| {
            recipes::table
                .inner_join(
                    recipe_versions::table.on(recipe_versions::id
                        .nullable()
                        .eq(recipes::current_version_id)),
                )
                .filter(recipes::id.eq(recipe_id))
                .filter(recipes::user_id.eq(user_id))
                .filter(recipes::deleted_at.is_null())
                .select((recipes::current_version_id, RecipeVersion::as_select()))
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

    let original_description = current_version.description.clone();

    let ai_client = ai_client_from_env()?;

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
            return Err(ApiError::service_unavailable(format!(
                "Description generation failed: {}",
                e
            )));
        }
    };

    let new_description = result.description.trim().to_string();
    let changed =
        !new_description.is_empty() && original_description.as_deref() != Some(&new_description);

    if !changed {
        return Ok((
            StatusCode::OK,
            Json(GenerateDescriptionResponse {
                original_description: original_description.clone(),
                generated_description: new_description,
                changed: false,
                cached: result.cached,
            }),
        ));
    }

    let description_for_write = new_description.clone();
    run_db(&pool, move |conn| {
        let write_result: Result<(), VersionWriteError> = conn.transaction(|conn| {
            let new_version = NewRecipeVersion {
                description: Some(&description_for_write),
                ..NewRecipeVersion::copy_of(&current_version, "generate_description")
            };

            // Compare-and-swap: only repoint if current_version_id hasn't changed
            // since our initial read, preventing overwrites of concurrent edits.
            create_new_version_cas(
                conn,
                &new_version,
                version_id_snapshot,
                crate::recipes::TagSource::CopyFrom(current_version.id),
            )?;

            Ok(())
        });

        match write_result {
            Ok(()) => Ok(()),
            Err(VersionWriteError::Stale) => Err(ApiError::conflict(
                "Recipe was modified while generating description; try again",
            )),
            Err(VersionWriteError::Db(e)) => {
                tracing::error!("Failed to persist generated description: {}", e);
                Err(ApiError::internal(
                    "Failed to persist generated description",
                ))
            }
        }
    })
    .await?;

    Ok((
        StatusCode::OK,
        Json(GenerateDescriptionResponse {
            original_description,
            generated_description: new_description,
            changed: true,
            cached: result.cached,
        }),
    ))
}
