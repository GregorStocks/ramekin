use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::{Ingredient, NewRecipeVersion, RecipeVersion};
use crate::recipes::{create_new_version_cas, TagSource, VersionWriteError};
use crate::schema::{recipe_versions, recipes};
use axum::{extract::State, Json};
use diesel::prelude::*;
use ramekin_core::ingredient_parser::{reparse_stored, ParsedIngredient};
use ramekin_core::volume_to_weight::enrich_ingredient_measurements;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ReparseIngredientsResponse {
    /// Recipes whose current version was checked.
    pub recipes_checked: usize,
    /// Recipes that got a new version because an ingredient changed.
    pub recipes_updated: usize,
    /// Ingredient lines whose item changed.
    pub ingredients_changed: usize,
}

/// The ingredients re-parsed with the current parser, and how many changed.
fn reparse_ingredients(ingredients: Vec<Ingredient>) -> Result<(Vec<Ingredient>, usize), ApiError> {
    let invalid = |_| ApiError::internal("Stored ingredients are not valid");
    let mut changed = 0;
    let updated = ingredients
        .into_iter()
        .map(|ingredient| {
            let parsed: ParsedIngredient =
                serde_json::from_value(serde_json::to_value(&ingredient).map_err(invalid)?)
                    .map_err(invalid)?;
            match reparse_stored(&parsed) {
                Some(reparsed) => {
                    changed += 1;
                    let enriched = enrich_ingredient_measurements(reparsed);
                    serde_json::from_value(serde_json::to_value(&enriched).map_err(invalid)?)
                        .map_err(invalid)
                }
                None => Ok(ingredient),
            }
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok((updated, changed))
}

#[utoipa::path(
    post,
    path = "/api/recipes/reparse-ingredients",
    tag = "recipes",
    responses(
        (status = 200, description = "Every recipe re-parsed; changed recipes got a new version", body = ReparseIngredientsResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 409, description = "A recipe changed while re-parsing; run it again", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
/// Re-parse the stored ingredients of all of the user's recipes with the
/// current parser, saving a new version (source "reparse") for each recipe
/// whose ingredients change.
pub async fn reparse_all_ingredients(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
) -> Result<Json<ReparseIngredientsResponse>, ApiError> {
    let user_id = user.id;
    let response = run_db(&pool, move |conn| {
        let versions: Vec<RecipeVersion> = recipes::table
            .inner_join(
                recipe_versions::table
                    .on(recipes::current_version_id.eq(recipe_versions::id.nullable())),
            )
            .filter(recipes::user_id.eq(user_id))
            .filter(recipes::deleted_at.is_null())
            .select(RecipeVersion::as_select())
            .load(conn)
            .map_err(|e| {
                tracing::error!("Failed to load recipes for re-parse: {}", e);
                ApiError::internal("Failed to load recipes")
            })?;

        let mut response = ReparseIngredientsResponse {
            recipes_checked: versions.len(),
            recipes_updated: 0,
            ingredients_changed: 0,
        };
        for current in &versions {
            let ingredients: Vec<Ingredient> = serde_json::from_value(current.ingredients.clone())
                .map_err(|_| ApiError::internal("Stored ingredients are not valid"))?;
            let (updated, changed) = reparse_ingredients(ingredients)?;
            if changed == 0 {
                continue;
            }
            let ingredients = serde_json::to_value(&updated)
                .map_err(|_| ApiError::internal("Failed to serialize ingredients"))?;
            let result: Result<_, VersionWriteError> = conn.transaction(|conn| {
                create_new_version_cas(
                    conn,
                    &NewRecipeVersion {
                        ingredients,
                        ..NewRecipeVersion::copy_of(current, "reparse")
                    },
                    Some(current.id),
                    TagSource::CopyFrom(current.id),
                )?;
                crate::ingredient_names::enqueue_items(
                    conn,
                    updated.iter().map(|i| i.item.as_str()),
                )?;
                Ok(())
            });
            match result {
                Ok(_) => {
                    response.recipes_updated += 1;
                    response.ingredients_changed += changed;
                }
                Err(VersionWriteError::Stale) => {
                    return Err(ApiError::conflict(
                        "A recipe changed while re-parsing; run it again",
                    ))
                }
                Err(VersionWriteError::Db(e)) => {
                    tracing::error!("Failed to save re-parsed recipe: {}", e);
                    return Err(ApiError::internal("Failed to save re-parsed recipe"));
                }
            }
        }
        tracing::info!(
            user_id = %user_id,
            recipes_checked = response.recipes_checked,
            recipes_updated = response.recipes_updated,
            ingredients_changed = response.ingredients_changed,
            "re-parsed stored ingredients"
        );
        Ok(response)
    })
    .await?;
    crate::ingredient_names::wake();
    Ok(Json(response))
}
