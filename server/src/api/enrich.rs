use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::schema::user_tags;
use crate::types::RecipeContent;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use ramekin_core::ai::{custom_enrich, suggest_tags, CachingAiClient};
use serde::Deserialize;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa::ToSchema;

/// Enrich a recipe using AI
///
/// This is a stateless endpoint that takes a recipe object and returns an enriched version.
/// It does NOT modify any database records. The client can apply the enriched data
/// via a normal PUT /api/recipes/{id} call.
///
/// Currently enriches tags by suggesting from the user's existing tag library.
#[utoipa::path(
    post,
    path = "/api/enrich",
    tag = "enrich",
    request_body = RecipeContent,
    responses(
        (status = 200, description = "Enriched recipe object", body = RecipeContent),
        (status = 401, description = "Unauthorized", body = crate::api::ErrorResponse),
        (status = 503, description = "AI service unavailable", body = crate::api::ErrorResponse)
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
    // Fetch user's existing tags from user_tags table
    let mut conn = get_conn!(pool);
    let user_tags: Vec<String> = user_tags::table
        .filter(user_tags::user_id.eq(user.id))
        .select(user_tags::name)
        .order(user_tags::name.asc())
        .load(&mut conn)
        .unwrap_or_default();

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

    // Format ingredients as string for prompt
    let ingredients_str = request
        .ingredients
        .iter()
        .map(|i| {
            // Format primary measurement if present
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

    // Call shared suggest_tags function
    let result = match suggest_tags(
        &ai_client,
        &request.title,
        &ingredients_str,
        &request.instructions,
        &user_tags,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("AI call failed: {}", e);
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("AI service error: {}", e),
                }),
            )
                .into_response();
        }
    };

    // Merge suggested tags with existing (dedup, case-insensitive)
    let mut tags = request.tags.clone();
    for tag in result.suggested_tags {
        if !tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
            tags.push(tag);
        }
    }

    // Return enriched recipe
    let enriched = RecipeContent { tags, ..request };
    (StatusCode::OK, Json(enriched)).into_response()
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
