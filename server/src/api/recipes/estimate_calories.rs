use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::models::Ingredient;
use axum::Json;
use ramekin_core::ingredient_parser::{Measurement, ParsedIngredient};
use ramekin_core::nutrition;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
pub struct EstimateCaloriesRequest {
    pub ingredients: Vec<Ingredient>,
    /// Original, unscaled serving count. Yield text is not interpreted as a count.
    pub servings: Option<String>,
    /// Multiplier applied to the whole recipe, including its serving count.
    pub scale: f64,
}

#[derive(Serialize, ToSchema)]
pub struct CalorieRange {
    pub min: f64,
    pub max: f64,
}

impl From<nutrition::CalorieRange> for CalorieRange {
    fn from(range: nutrition::CalorieRange) -> Self {
        Self {
            min: range.min,
            max: range.max,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct UnknownCalorieIngredient {
    pub index: usize,
    pub item: String,
    pub reason: String,
}

#[derive(Serialize, ToSchema)]
pub struct CalorieEstimateResponse {
    /// Identifies the pinned source data, aliases, and calculation rules.
    pub database_version: String,
    /// Null when no ingredient could be estimated. Otherwise a subtotal that may be partial.
    pub known_calories: Option<CalorieRange>,
    pub per_serving_calories: Option<CalorieRange>,
    pub unknown_ingredients: Vec<UnknownCalorieIngredient>,
    pub summary: String,
    pub per_serving_summary: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/recipes/estimate-calories",
    tag = "recipes",
    request_body = EstimateCaloriesRequest,
    responses(
        (status = 200, description = "Deterministic calorie estimate", body = CalorieEstimateResponse),
        (status = 400, description = "Invalid scale or numeric bounds", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn estimate_calories(
    AuthUser(_user): AuthUser,
    Json(request): Json<EstimateCaloriesRequest>,
) -> Result<Json<CalorieEstimateResponse>, ApiError> {
    let ingredients = request
        .ingredients
        .into_iter()
        .map(|ingredient| ParsedIngredient {
            item: ingredient.item,
            measurements: ingredient
                .measurements
                .into_iter()
                .map(|m| Measurement {
                    amount: m.amount,
                    unit: m.unit,
                })
                .collect(),
            note: ingredient.note,
            section: ingredient.section,
            raw: None,
        })
        .collect::<Vec<_>>();
    let result = nutrition::estimate(&ingredients, request.servings.as_deref(), request.scale)
        .map_err(ApiError::invalid_request)?;
    Ok(Json(CalorieEstimateResponse {
        database_version: result.database_version,
        known_calories: result.known_calories.map(Into::into),
        per_serving_calories: result.per_serving_calories.map(Into::into),
        unknown_ingredients: result
            .unknown_ingredients
            .into_iter()
            .map(|item| UnknownCalorieIngredient {
                index: item.index,
                item: item.item,
                reason: item.reason,
            })
            .collect(),
        summary: result.summary,
        per_serving_summary: result.per_serving_summary,
    }))
}
