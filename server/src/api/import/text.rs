use std::{collections::HashMap, error::Error, sync::Arc};

use async_trait::async_trait;
use axum::{extract::State, Json};
use diesel::prelude::*;
use ramekin_core::ai::{text_extract::extract_recipe_from_text, AiClient, CachingAiClient};
use ramekin_core::pipeline::{
    scrape_auto_applied_ai_enrichments, steps::EnrichAutoTagStep,
    steps::EnrichGenerateDescriptionStep, steps::EnrichNormalizeTitleStep,
    steps::ParseIngredientsStep, PipelineStep, ScrapeAutoAppliedAiEnrichment, StepContext,
    StepOutputStore,
};
use ramekin_core::RawRecipe;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;

use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::Ingredient;
use crate::schema::user_tags;
use crate::types::RecipeContent;

#[derive(Deserialize, ToSchema)]
pub struct PrepareTextRecipeRequest {
    /// A whole recipe, typed or pasted as unstructured text.
    pub text: String,
}

#[derive(Serialize, ToSchema)]
pub struct PrepareTextRecipeResponse {
    pub content: RecipeContent,
    /// Editable ingredient lines. Send these as raw_ingredients when saving.
    pub raw_ingredients: String,
    pub warnings: Vec<String>,
}

/// Draft execution uses the same pipeline steps without persisting a recipe.
#[derive(Default)]
struct DraftOutputs(HashMap<String, Value>);

#[async_trait]
impl StepOutputStore for DraftOutputs {
    async fn get_output(&self, name: &str) -> Option<Value> {
        self.0.get(name).cloned()
    }

    async fn save_output(
        &mut self,
        name: &str,
        output: &Value,
        _duration_ms: i64,
        _success: bool,
        _error: Option<&str>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.0.insert(name.to_string(), output.clone());
        Ok(())
    }
}

impl DraftOutputs {
    async fn execute(&mut self, step: &dyn PipelineStep) -> Result<Value, ApiError> {
        let result = step
            .execute(&StepContext {
                url: "",
                outputs: self,
            })
            .await;
        if !result.success {
            tracing::warn!(step = result.step_name, error = ?result.error, "Recipe draft failed");
            return Err(ApiError::service_unavailable(
                "Recipe processing failed. Your text is unchanged; try reviewing it again.",
            ));
        }
        self.0.insert(result.step_name, result.output.clone());
        Ok(result.output)
    }
}

pub(crate) async fn parse_draft_ingredients(raw: &RawRecipe) -> Result<Vec<Ingredient>, ApiError> {
    let mut outputs = DraftOutputs::default();
    outputs
        .0
        .insert("extract_recipe".into(), json!({"raw_recipe": raw}));
    let parsed = outputs.execute(&ParseIngredientsStep).await?;
    serde_json::from_value(parsed["ingredients"].clone())
        .map_err(|_| ApiError::internal("Invalid ingredient pipeline output"))
}

#[utoipa::path(
    post, path = "/api/import/text", tag = "import",
    request_body = PrepareTextRecipeRequest,
    responses(
        (status = 200, description = "Recipe draft ready for review; nothing saved", body = PrepareTextRecipeResponse),
        (status = 400, description = "Invalid recipe text", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 503, description = "Recipe processing failed", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn prepare_text_recipe(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<PrepareTextRecipeRequest>,
) -> Result<Json<PrepareTextRecipeResponse>, ApiError> {
    if request.text.trim().is_empty() || request.text.len() > 50_000 {
        return Err(ApiError::invalid_request(
            "Enter recipe text between 1 and 50,000 bytes.",
        ));
    }
    let client: Arc<dyn AiClient> = Arc::new(CachingAiClient::from_env().map_err(|e| {
        tracing::warn!(error = %e, "Text extraction unavailable");
        ApiError::service_unavailable("Recipe text processing is unavailable. Ask your server administrator to configure AI access, then retry.")
    })?);
    let extracted = extract_recipe_from_text(client.as_ref(), &request.text)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "Text extraction failed");
            ApiError::service_unavailable(
                "Could not read the recipe. Your text is unchanged; try again.",
            )
        })?;
    let raw = extracted.raw_recipe;
    let mut warnings = extracted.warnings;
    for (name, value) in [
        ("title", &raw.title),
        ("ingredients", &raw.ingredients),
        ("instructions", &raw.instructions),
    ] {
        if value.trim().is_empty() {
            warnings.push(format!("Missing {name}. Add it before saving."));
        }
    }
    let ingredients = parse_draft_ingredients(&raw).await?;
    if ingredients.is_empty() && !raw.ingredients.trim().is_empty() {
        warnings.push("No ingredients found. Add ingredient lines before saving.".to_string());
    }
    let mut value =
        serde_json::to_value(&raw).map_err(|_| ApiError::internal("Invalid extracted recipe"))?;
    value["ingredients"] = json!(ingredients);
    value["tags"] = json!(raw.categories.clone().unwrap_or_default());
    let mut content: RecipeContent = serde_json::from_value(value)
        .map_err(|_| ApiError::internal("Invalid extracted recipe"))?;
    // Incomplete/ambiguous recipes need correction before AI enrichment can be grounded.
    if warnings.is_empty() {
        let tags = run_db(&pool, move |conn| {
            user_tags::table
                .filter(user_tags::user_id.eq(user.id))
                .filter(user_tags::deleted_at.is_null())
                .select(user_tags::name)
                .order(user_tags::name.asc())
                .load::<String>(conn)
                .map_err(|_| ApiError::internal("Could not load recipe tags"))
        })
        .await?;
        let mut outputs = DraftOutputs::default();
        outputs
            .0
            .insert("extract_recipe".into(), json!({"raw_recipe": raw}));
        for enrichment in scrape_auto_applied_ai_enrichments() {
            match enrichment {
                ScrapeAutoAppliedAiEnrichment::NormalizeTitle => {
                    let output = outputs
                        .execute(&EnrichNormalizeTitleStep::new(client.clone()))
                        .await?;
                    if output["changed"] == true {
                        content.title = required_string(&output, "normalized_title")?;
                    }
                }
                ScrapeAutoAppliedAiEnrichment::GenerateDescription => {
                    let output = outputs
                        .execute(&EnrichGenerateDescriptionStep::new(client.clone()))
                        .await?;
                    if output["changed"] == true {
                        content.description =
                            Some(required_string(&output, "generated_description")?);
                    }
                }
                ScrapeAutoAppliedAiEnrichment::AutoTag => {
                    let output = outputs
                        .execute(&EnrichAutoTagStep::new(client.clone(), tags.clone()))
                        .await?;
                    let suggested: Vec<String> =
                        serde_json::from_value(output["suggested_tags"].clone())
                            .map_err(|_| ApiError::internal("Invalid tag pipeline output"))?;
                    for tag in suggested {
                        if !content.tags.contains(&tag) {
                            content.tags.push(tag);
                        }
                    }
                }
            }
        }
    }
    for ingredient in &content.ingredients {
        let has_weight = ingredient.measurements.iter().any(|measurement| {
            measurement.amount.is_some()
                && matches!(
                    measurement.unit.as_deref(),
                    Some("g" | "kg" | "mg" | "oz" | "lb")
                )
        });
        if !has_weight {
            warnings.push(format!(
                "Weight estimate unavailable for {}.",
                ingredient.item
            ));
        }
    }
    Ok(Json(PrepareTextRecipeResponse {
        content,
        raw_ingredients: raw.ingredients,
        warnings,
    }))
}

fn required_string(output: &Value, field: &str) -> Result<String, ApiError> {
    output[field]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ApiError::internal("Invalid enrichment pipeline output"))
}
