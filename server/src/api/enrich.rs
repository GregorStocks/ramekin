use crate::auth::AuthUser;
use crate::enrichment::{self, EnrichmentError, EnrichmentInfo};
use crate::llm;
use crate::types::RecipeContent;
use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use super::ErrorResponse;

/// Request to enrich a recipe.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct EnrichRequest {
    /// The type of enrichment to apply (e.g., "normalize_ingredients").
    pub enrichment_type: String,
    /// The recipe content to enrich.
    pub recipe: RecipeContent,
}

/// Response from the list enrichments endpoint.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ListEnrichmentsResponse {
    /// Available enrichment types.
    pub enrichments: Vec<EnrichmentInfo>,
}

/// Enrich a recipe using AI
///
/// This is a stateless endpoint that takes a recipe object and returns an enriched version.
/// It does NOT modify any database records. The client can apply the enriched data
/// via a normal PUT /api/recipes/{id} call.
#[utoipa::path(
    post,
    path = "/api/enrich",
    tag = "enrich",
    request_body = EnrichRequest,
    responses(
        (status = 200, description = "Enriched recipe object", body = RecipeContent),
        (status = 400, description = "Invalid enrichment type", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 503, description = "AI service unavailable", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn enrich_recipe(
    AuthUser(_user): AuthUser,
    Json(request): Json<EnrichRequest>,
) -> impl IntoResponse {
    // Look up the enrichment type
    let enrichment = match enrichment::get_enrichment(&request.enrichment_type) {
        Some(e) => e,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Unknown enrichment type: {}", request.enrichment_type),
                }),
            )
                .into_response()
        }
    };

    // Get the LLM provider (with caching for efficiency)
    let provider = match llm::create_cached_provider_from_env() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create LLM provider");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("AI service unavailable: {}", e),
                }),
            )
                .into_response();
        }
    };

    // Run the enrichment
    match enrichment.run(provider.as_ref(), &request.recipe).await {
        Ok(enriched) => (StatusCode::OK, Json(enriched)).into_response(),
        Err(e) => {
            tracing::warn!(
                enrichment_type = enrichment.enrichment_type(),
                error = %e,
                "Enrichment failed"
            );
            let (status, message) = match e {
                EnrichmentError::Llm(llm::LlmError::RateLimited { .. }) => {
                    (StatusCode::TOO_MANY_REQUESTS, e.to_string())
                }
                EnrichmentError::Llm(llm::LlmError::NotConfigured(_)) => {
                    (StatusCode::SERVICE_UNAVAILABLE, e.to_string())
                }
                _ => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            };
            (status, Json(ErrorResponse { error: message })).into_response()
        }
    }
}

/// List available enrichment types
///
/// Returns information about all available enrichment types, including their
/// names, descriptions, and which recipe fields they modify.
#[utoipa::path(
    get,
    path = "/api/enrichments",
    tag = "enrich",
    responses(
        (status = 200, description = "List of available enrichments", body = ListEnrichmentsResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_enrichments(AuthUser(_user): AuthUser) -> impl IntoResponse {
    let enrichments = enrichment::all_enrichment_info();
    (
        StatusCode::OK,
        Json(ListEnrichmentsResponse { enrichments }),
    )
        .into_response()
}

#[derive(OpenApi)]
#[openapi(
    paths(enrich_recipe, list_enrichments),
    components(schemas(EnrichRequest, ListEnrichmentsResponse, EnrichmentInfo, RecipeContent))
)]
pub struct ApiDoc;
