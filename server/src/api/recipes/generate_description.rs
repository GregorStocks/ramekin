use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::{Ingredient, NewRecipeVersion};
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
    path = "/api/recipes/{id}/generate-description",
    tag = "recipes",
    params(
        ("id" = Uuid, Path, description = "Recipe ID")
    ),
    responses(
        (status = 200, description = "Description generated and applied", body = GenerateDescriptionResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Recipe not found", body = ErrorResponse),
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
        Err(diesel::NotFound) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Recipe not found".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to fetch recipe: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch recipe".to_string(),
                }),
            )
                .into_response();
        }
    };

    let (
        recipe_id,
        title,
        original_description,
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

    // Snapshot the current version ID before the AI call so we can detect
    // concurrent edits inside the write transaction.
    let version_id_before_ai: Option<Uuid> = recipes::table
        .filter(recipes::id.eq(recipe_id))
        .select(recipes::current_version_id)
        .first(&mut conn)
        .unwrap_or(None);

    let ai_client = match CachingAiClient::from_env() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("AI client unavailable: {}", e);
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "AI service unavailable".to_string(),
                }),
            )
                .into_response();
        }
    };

    let ingredients_str = format_ingredients_for_prompt(&ingredients);

    let result =
        match ai_generate_description(&ai_client, &title, &ingredients_str, &instructions).await {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("generate_description call failed: {}", e);
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(ErrorResponse {
                        error: format!("Description generation failed: {}", e),
                    }),
                )
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

    let write_result: Result<(), diesel::result::Error> = conn.transaction(|conn| {
        let new_version = NewRecipeVersion {
            recipe_id,
            title: &title,
            description: Some(&new_description),
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
            version_source: "generate_description",
        };

        let old_version_id: Option<Uuid> = recipes::table
            .filter(recipes::id.eq(recipe_id))
            .select(recipes::current_version_id)
            .first(conn)?;

        // Abort if the recipe was edited while the AI call was in flight.
        if old_version_id != version_id_before_ai {
            return Err(diesel::result::Error::RollbackTransaction);
        }

        let new_version_id: Uuid = diesel::insert_into(recipe_versions::table)
            .values(&new_version)
            .returning(recipe_versions::id)
            .get_result(conn)?;

        diesel::update(recipes::table.find(recipe_id))
            .set(recipes::current_version_id.eq(new_version_id))
            .execute(conn)?;

        // Carry over tags from the previous version
        if let Some(old_vid) = old_version_id {
            use crate::schema::recipe_version_tags;
            let old_tag_ids: Vec<Uuid> = recipe_version_tags::table
                .filter(recipe_version_tags::recipe_version_id.eq(old_vid))
                .select(recipe_version_tags::tag_id)
                .load(conn)?;
            for tag_id in old_tag_ids {
                diesel::insert_into(recipe_version_tags::table)
                    .values(crate::models::RecipeVersionTag {
                        recipe_version_id: new_version_id,
                        tag_id,
                    })
                    .on_conflict_do_nothing()
                    .execute(conn)?;
            }
        }

        Ok(())
    });

    if let Err(e) = write_result {
        if matches!(e, diesel::result::Error::RollbackTransaction) {
            return (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: "Recipe was modified while generating description; try again"
                        .to_string(),
                }),
            )
                .into_response();
        }
        tracing::error!("Failed to persist generated description: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to persist generated description".to_string(),
            }),
        )
            .into_response();
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
