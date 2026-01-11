use crate::auth::AuthUser;
use crate::types::RecipeContent;
use axum::{http::StatusCode, response::IntoResponse, Json};
use utoipa::OpenApi;

/// Enrich a recipe using AI
///
/// This is a stateless endpoint that takes a recipe object and returns an enriched version.
/// It does NOT modify any database records. The client can apply the enriched data
/// via a normal PUT /api/recipes/{id} call.
///
/// Currently a no-op skeleton - returns the input unchanged.
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
    AuthUser(_user): AuthUser,
    Json(request): Json<RecipeContent>,
) -> impl IntoResponse {
    // No-op skeleton: just return the input unchanged
    // TODO: Call Claude API for actual enrichment
    (StatusCode::OK, Json(request)).into_response()
}

#[derive(OpenApi)]
#[openapi(paths(enrich_recipe), components(schemas(RecipeContent)))]
pub struct ApiDoc;
