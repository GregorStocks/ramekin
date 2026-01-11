use crate::models::Ingredient;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Core recipe content - all fields that can be enriched by AI.
/// Used for: Enrich API request/response, base for CreateRecipeRequest.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RecipeContent {
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
