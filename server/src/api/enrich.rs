use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::models::Ingredient;
use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

/// Request body for enrichment - a recipe object to enhance
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct EnrichRequest {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub ingredients: Vec<Ingredient>,
    pub instructions: String,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub source_name: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub servings: Option<String>,
    #[serde(default)]
    pub prep_time: Option<String>,
    #[serde(default)]
    pub cook_time: Option<String>,
    #[serde(default)]
    pub total_time: Option<String>,
    #[serde(default)]
    pub rating: Option<i32>,
    #[serde(default)]
    pub difficulty: Option<String>,
    #[serde(default)]
    pub nutritional_info: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Response from enrichment - the enhanced recipe object
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EnrichResponse {
    pub title: String,
    pub description: Option<String>,
    pub ingredients: Vec<Ingredient>,
    pub instructions: String,
    pub source_url: Option<String>,
    pub source_name: Option<String>,
    pub tags: Vec<String>,
    pub servings: Option<String>,
    pub prep_time: Option<String>,
    pub cook_time: Option<String>,
    pub total_time: Option<String>,
    pub rating: Option<i32>,
    pub difficulty: Option<String>,
    pub nutritional_info: Option<String>,
    pub notes: Option<String>,
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
        (status = 200, description = "Enriched recipe object", body = EnrichResponse),
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
    // Check if AI API key is configured
    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "AI enrichment not configured (ANTHROPIC_API_KEY not set)".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Call Claude to enrich the recipe
    match call_claude_for_enrichment(&api_key, &request).await {
        Ok(enriched) => (StatusCode::OK, Json(enriched)).into_response(),
        Err(e) => {
            tracing::error!("AI enrichment failed: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("AI enrichment failed: {}", e),
                }),
            )
                .into_response()
        }
    }
}

/// Call Claude API to enrich a recipe
async fn call_claude_for_enrichment(
    api_key: &str,
    request: &EnrichRequest,
) -> Result<EnrichResponse, String> {
    let client = reqwest::Client::new();

    // Format the recipe for the prompt
    let ingredients_text = request
        .ingredients
        .iter()
        .map(|i| i.item.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        r#"You are a culinary expert helping to enhance and enrich recipe data.

Given this recipe, please enhance it by:
1. Improving the description if it's missing or sparse
2. Suggesting appropriate tags/categories
3. Estimating prep_time, cook_time, and total_time if missing
4. Suggesting a difficulty level (easy, medium, hard) if missing
5. Cleaning up ingredient formatting
6. Improving instruction clarity and formatting

Current recipe:
Title: {title}
Description: {description}
Ingredients:
{ingredients}

Instructions:
{instructions}

Current tags: {tags}
Servings: {servings}
Prep time: {prep_time}
Cook time: {cook_time}
Total time: {total_time}
Difficulty: {difficulty}
Notes: {notes}

Please respond with a JSON object containing the enriched recipe. Include ALL fields, even if unchanged:
{{
  "title": "...",
  "description": "...",
  "ingredients": [{{ "item": "..." }}, ...],
  "instructions": "...",
  "tags": ["...", ...],
  "servings": "...",
  "prep_time": "...",
  "cook_time": "...",
  "total_time": "...",
  "difficulty": "...",
  "notes": "..."
}}

Respond ONLY with the JSON object, no other text."#,
        title = request.title,
        description = request.description.as_deref().unwrap_or("(none)"),
        ingredients = ingredients_text,
        instructions = request.instructions,
        tags = request.tags.join(", "),
        servings = request.servings.as_deref().unwrap_or("(none)"),
        prep_time = request.prep_time.as_deref().unwrap_or("(none)"),
        cook_time = request.cook_time.as_deref().unwrap_or("(none)"),
        total_time = request.total_time.as_deref().unwrap_or("(none)"),
        difficulty = request.difficulty.as_deref().unwrap_or("(none)"),
        notes = request.notes.as_deref().unwrap_or("(none)"),
    );

    let request_body = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 4096,
        "messages": [
            {
                "role": "user",
                "content": prompt
            }
        ]
    });

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "(failed to read body)".to_string());
        return Err(format!("Claude API returned {}: {}", status, body));
    }

    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Claude response: {}", e))?;

    // Extract the text content from Claude's response
    let content = response_json["content"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|c| c["text"].as_str())
        .ok_or("No text content in Claude response")?;

    // Parse the JSON from Claude's response
    let enriched: EnrichedRecipeFromAI =
        serde_json::from_str(content).map_err(|e| format!("Failed to parse AI response: {}", e))?;

    Ok(EnrichResponse {
        title: enriched.title,
        description: enriched.description,
        ingredients: enriched.ingredients,
        instructions: enriched.instructions,
        source_url: request.source_url.clone(),
        source_name: request.source_name.clone(),
        tags: enriched.tags,
        servings: enriched.servings,
        prep_time: enriched.prep_time,
        cook_time: enriched.cook_time,
        total_time: enriched.total_time,
        rating: request.rating, // Keep original rating
        difficulty: enriched.difficulty,
        nutritional_info: request.nutritional_info.clone(), // Keep original
        notes: enriched.notes,
    })
}

/// Intermediate struct for parsing AI response (some fields may be null/absent)
#[derive(Debug, Deserialize)]
struct EnrichedRecipeFromAI {
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    ingredients: Vec<Ingredient>,
    instructions: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    servings: Option<String>,
    #[serde(default)]
    prep_time: Option<String>,
    #[serde(default)]
    cook_time: Option<String>,
    #[serde(default)]
    total_time: Option<String>,
    #[serde(default)]
    difficulty: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(OpenApi)]
#[openapi(
    paths(enrich_recipe),
    components(schemas(EnrichRequest, EnrichResponse))
)]
pub struct ApiDoc;
