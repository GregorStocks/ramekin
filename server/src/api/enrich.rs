use crate::api::ApiError;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::Ingredient;
use crate::photos::{load_photo_images, PhotoImageLoadError};
use crate::schema::user_tags;
use crate::types::RecipeContent;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use ramekin_core::ai::{custom_enrich, suggest_tags, CachingAiClient};
use ramekin_core::enrich_ingredient_measurements;
use ramekin_core::ingredient_parser::ParsedIngredient;
use serde::Deserialize;
use std::fmt;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa::ToSchema;

/// Enrich ingredient measurements by adding gram conversions.
///
/// Converts volume units (cups, tbsp, tsp) and imperial weights (oz, lb)
/// to grams when density data is available.
fn enrich_ingredients(ingredients: Vec<Ingredient>) -> Result<Vec<Ingredient>, serde_json::Error> {
    ingredients
        .into_iter()
        .map(|ing| {
            let parsed: ParsedIngredient = serde_json::from_value(serde_json::to_value(&ing)?)?;
            let enriched = enrich_ingredient_measurements(parsed);
            serde_json::from_value(serde_json::to_value(&enriched)?)
        })
        .collect()
}

#[derive(Debug)]
enum TagEnrichmentError {
    Database(String),
    Ai(String),
}

impl TagEnrichmentError {
    fn into_api_error(self) -> ApiError {
        match self {
            TagEnrichmentError::Database(e) => {
                tracing::error!("Tag enrichment failed while reading user tags: {}", e);
                ApiError::internal("Failed to enrich tags")
            }
            TagEnrichmentError::Ai(e) => {
                tracing::warn!("AI tag enrichment unavailable: {}", e);
                ApiError::service_unavailable("AI service unavailable")
            }
        }
    }
}

impl fmt::Display for TagEnrichmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TagEnrichmentError::Database(e) => write!(f, "database error: {}", e),
            TagEnrichmentError::Ai(e) => write!(f, "AI error: {}", e),
        }
    }
}

/// Enrich a recipe
///
/// This is a stateless endpoint that takes a recipe object and returns an enriched version.
/// It does NOT modify any database records. The client can apply the enriched data
/// via a normal PUT /api/recipes/{id} call.
///
/// Enriches:
/// - Ingredient measurements with gram conversions (volume/weight → grams)
/// - Tags by suggesting from the user's existing tag library (requires AI; returns 503 if unavailable)
#[utoipa::path(
    post,
    path = "/api/enrich",
    tag = "enrich",
    request_body = RecipeContent,
    responses(
        (status = 200, description = "Enriched recipe object", body = RecipeContent),
        (status = 401, description = "Unauthorized", body = crate::api::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::api::ErrorResponse),
        (status = 503, description = "AI service unavailable", body = crate::api::ErrorResponse),
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
    let tags = match try_enrich_tags(&user.id, &pool, &request).await {
        Ok(tags) => tags,
        Err(e) => return e.into_api_error().into_response(),
    };

    // Enrich ingredient measurements (no AI needed - uses density database)
    let ingredients = match enrich_ingredients(request.ingredients) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to enrich ingredients: {}", e);
            return ApiError::internal("Failed to enrich ingredients").into_response();
        }
    };

    // Return enriched recipe
    let enriched = RecipeContent {
        tags,
        ingredients,
        ..request
    };
    (StatusCode::OK, Json(enriched)).into_response()
}

/// Try to enrich tags using AI.
async fn try_enrich_tags(
    user_id: &uuid::Uuid,
    pool: &Arc<DbPool>,
    request: &RecipeContent,
) -> Result<Vec<String>, TagEnrichmentError> {
    let mut conn = pool
        .get()
        .map_err(|e| TagEnrichmentError::Database(e.to_string()))?;
    let user_tags: Vec<String> = user_tags::table
        .filter(user_tags::user_id.eq(user_id))
        .filter(user_tags::deleted_at.is_null())
        .select(user_tags::name)
        .order(user_tags::name.asc())
        .load(&mut conn)
        .map_err(|e| TagEnrichmentError::Database(format!("failed to fetch user tags: {}", e)))?;

    let ai_client =
        CachingAiClient::from_env().map_err(|e| TagEnrichmentError::Ai(e.to_string()))?;

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
    .map_err(|e| TagEnrichmentError::Ai(e.to_string()))?;

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
    #[serde(default)]
    pub photo_ids: Vec<uuid::Uuid>,
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
        (status = 400, description = "Invalid photo IDs", body = crate::api::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::api::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::api::ErrorResponse),
        (status = 503, description = "AI service unavailable", body = crate::api::ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn custom_enrich_recipe(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<CustomEnrichRequest>,
) -> impl IntoResponse {
    // Create AI client
    let ai_client = match CachingAiClient::from_env() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("AI client unavailable: {}", e);
            return ApiError::service_unavailable("AI service unavailable").into_response();
        }
    };

    // Serialize the recipe to JSON for the prompt
    let recipe_json = match serde_json::to_string_pretty(&request.recipe) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("Failed to serialize recipe for AI: {}", e);
            return ApiError::internal("Failed to serialize recipe").into_response();
        }
    };

    let images = match load_photo_images(&pool, user.id, &request.photo_ids) {
        Ok(images) => images,
        Err(PhotoImageLoadError::NotFound) => {
            return ApiError::invalid_request(
                "One or more photo_ids not found or don't belong to user",
            )
            .into_response();
        }
        Err(PhotoImageLoadError::Database(e)) => {
            tracing::error!("Failed to load custom enrich photos: {}", e);
            return ApiError::internal("Failed to load photos").into_response();
        }
    };

    // Call custom enrich
    let result = match custom_enrich(&ai_client, &recipe_json, &request.instruction, images).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Custom enrich AI call failed: {}", e);
            return ApiError::service_unavailable(format!("AI service error: {}", e))
                .into_response();
        }
    };

    // Deserialize the AI response back into RecipeContent
    let modified: RecipeContent = match serde_json::from_str(&result.recipe_json) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to parse AI response: {}", e);
            return ApiError::service_unavailable(format!("Failed to parse AI response: {}", e))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ErrorCode;

    #[test]
    fn database_tag_enrichment_error_maps_to_internal() {
        let error = TagEnrichmentError::Database("connection refused".to_string()).into_api_error();

        assert_eq!(error.code, ErrorCode::Internal);
        assert_eq!(error.message, "Failed to enrich tags");
    }

    #[test]
    fn ai_tag_enrichment_error_maps_to_service_unavailable() {
        let error = TagEnrichmentError::Ai(
            "Missing required environment variable: OPENROUTER_API_KEY".to_string(),
        )
        .into_api_error();

        assert_eq!(error.code, ErrorCode::ServiceUnavailable);
        assert_eq!(error.message, "AI service unavailable");
    }
}
