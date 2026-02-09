use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::Ingredient;
use crate::schema::user_tags;
use crate::types::RecipeContent;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use ramekin_core::ai::{custom_enrich, suggest_tags, CachingAiClient};
use ramekin_core::enrich_ingredient_measurements;
use ramekin_core::ingredient_parser::ParsedIngredient;
use serde::Deserialize;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa::ToSchema;

/// Enrich ingredient measurements by adding gram conversions.
///
/// Converts volume units (cups, tbsp, tsp) and imperial weights (oz, lb)
/// to grams when density data is available.
fn enrich_ingredients(ingredients: Vec<Ingredient>) -> Vec<Ingredient> {
    ingredients
        .into_iter()
        .map(|ing| {
            let parsed: ParsedIngredient =
                serde_json::from_value(serde_json::to_value(&ing).unwrap()).unwrap();
            let enriched = enrich_ingredient_measurements(parsed);
            serde_json::from_value(serde_json::to_value(&enriched).unwrap()).unwrap()
        })
        .collect()
}

/// Enrich a recipe
///
/// This is a stateless endpoint that takes a recipe object and returns an enriched version.
/// It does NOT modify any database records. The client can apply the enriched data
/// via a normal PUT /api/recipes/{id} call.
///
/// Enriches:
/// - Ingredient measurements with gram conversions (volume/weight → grams)
/// - Tags by suggesting from the user's existing tag library (requires AI; skipped if unavailable)
#[utoipa::path(
    post,
    path = "/api/enrich",
    tag = "enrich",
    request_body = RecipeContent,
    responses(
        (status = 200, description = "Enriched recipe object", body = RecipeContent),
        (status = 401, description = "Unauthorized", body = crate::api::ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn enrich_recipe(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<RecipeContent>,
) -> impl IntoResponse {
    // Try AI-based tag enrichment (best-effort)
    let tags = match try_enrich_tags(&user.id, &pool, &request).await {
        Ok(tags) => tags,
        Err(e) => {
            tracing::warn!("AI tag enrichment skipped: {}", e);
            request.tags.clone()
        }
    };

    // Enrich ingredient measurements (no AI needed - uses density database)
    let ingredients = enrich_ingredients(request.ingredients);

    // Return enriched recipe
    let enriched = RecipeContent {
        tags,
        ingredients,
        ..request
    };
    (StatusCode::OK, Json(enriched)).into_response()
}

/// Try to enrich tags using AI. Returns the original tags on any failure.
async fn try_enrich_tags(
    user_id: &uuid::Uuid,
    pool: &Arc<DbPool>,
    request: &RecipeContent,
) -> Result<Vec<String>, String> {
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    let user_tags: Vec<String> = user_tags::table
        .filter(user_tags::user_id.eq(user_id))
        .filter(user_tags::deleted_at.is_null())
        .select(user_tags::name)
        .order(user_tags::name.asc())
        .load(&mut conn)
        .unwrap_or_default();

    let ai_client = CachingAiClient::from_env().map_err(|e| e.to_string())?;

    // Format ingredients as string for prompt
    let ingredients_str = request
        .ingredients
        .iter()
        .map(|i| {
            let measurement_str = i
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
            format!("{} {}", measurement_str, i.item).trim().to_string()
        })
        .collect::<Vec<_>>()
        .join(", ");

    let result = suggest_tags(
        &ai_client,
        &request.title,
        &ingredients_str,
        &request.instructions,
        &user_tags,
    )
    .await
    .map_err(|e| e.to_string())?;

    // Merge suggested tags with existing (dedup, case-insensitive)
    let mut tags = request.tags.clone();
    for tag in result.suggested_tags {
        if !tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
            tags.push(tag);
        }
    }

    Ok(tags)
}

/// Request body for custom enrichment.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CustomEnrichRequest {
    pub recipe: RecipeContent,
    pub instruction: String,
}

/// Apply a custom AI modification to a recipe
///
/// Takes a recipe and a free-text instruction describing the desired change.
/// Returns the complete modified recipe. Stateless - does NOT modify any database records.
#[utoipa::path(
    post,
    path = "/api/enrich/custom",
    tag = "enrich",
    request_body = CustomEnrichRequest,
    responses(
        (status = 200, description = "Modified recipe", body = RecipeContent),
        (status = 401, description = "Unauthorized", body = crate::api::ErrorResponse),
        (status = 503, description = "AI service unavailable", body = crate::api::ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn custom_enrich_recipe(
    AuthUser(_user): AuthUser,
    Json(request): Json<CustomEnrichRequest>,
) -> impl IntoResponse {
    // Create AI client
    let ai_client = match CachingAiClient::from_env() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("AI client unavailable: {}", e);
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "AI service unavailable".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Serialize the recipe to JSON for the prompt
    let recipe_json = serde_json::to_string_pretty(&request.recipe).unwrap();

    // Call custom enrich
    let result = match custom_enrich(&ai_client, &recipe_json, &request.instruction).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Custom enrich AI call failed: {}", e);
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("AI service error: {}", e),
                }),
            )
                .into_response();
        }
    };

    // Deserialize the AI response back into RecipeContent
    let modified: RecipeContent = match serde_json::from_str(&result.recipe_json) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to parse AI response: {}", e);
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("Failed to parse AI response: {}", e),
                }),
            )
                .into_response();
        }
    };

    (StatusCode::OK, Json(modified)).into_response()
}

#[derive(OpenApi)]
#[openapi(
    paths(enrich_recipe, custom_enrich_recipe),
    components(schemas(RecipeContent, CustomEnrichRequest))
)]
pub struct ApiDoc;
