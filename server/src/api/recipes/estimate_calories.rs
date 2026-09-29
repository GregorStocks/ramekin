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

/// How far to trust the estimate, from how many ingredients it couldn't count.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CalorieStatus {
    /// Every ingredient is counted or negligible.
    Complete,
    /// A few ingredients are missing, so the figures are lower bounds.
    Partial,
    /// Too many ingredients are missing to show a number.
    Insufficient,
    /// Nothing to estimate.
    Empty,
}

impl From<nutrition::Status> for CalorieStatus {
    fn from(status: nutrition::Status) -> Self {
        match status {
            nutrition::Status::Complete => Self::Complete,
            nutrition::Status::Partial => Self::Partial,
            nutrition::Status::Insufficient => Self::Insufficient,
            nutrition::Status::Empty => Self::Empty,
        }
    }
}

/// One ingredient line's part of the estimate.
#[derive(Serialize, ToSchema)]
pub struct CalorieLine {
    pub index: usize,
    pub item: String,
    /// Scaled calories when counted (zero when negligible).
    pub calories: Option<CalorieRange>,
    /// Display text: "~120 kcal", "Negligible", "Not a food", or why it
    /// couldn't be counted ("Not recognized", "Amount unclear").
    pub text: String,
}

/// Every display string is final; clients render them as-is.
#[derive(Serialize, ToSchema)]
pub struct CalorieEstimateResponse {
    /// Identifies the pinned source data, aliases, and calculation rules.
    pub database_version: String,
    pub status: CalorieStatus,
    /// The main line: "~520 kcal per serving", "At least ~3,100 kcal for the
    /// whole recipe", or "Not enough ingredient data to estimate calories".
    pub headline: String,
    /// Shown under the headline when present.
    pub secondary: Option<String>,
    /// For a partial estimate, the ingredients its lower bound leaves out.
    pub not_counted: Vec<String>,
    /// Null when nothing was counted. A lower bound when status is partial.
    pub known_calories: Option<CalorieRange>,
    pub per_serving_calories: Option<CalorieRange>,
    /// The breakdown, one entry per ingredient in order.
    pub lines: Vec<CalorieLine>,
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
        status: result.status.into(),
        headline: result.headline,
        secondary: result.secondary,
        not_counted: result.not_counted,
        known_calories: result.known_calories.map(Into::into),
        per_serving_calories: result.per_serving_calories.map(Into::into),
        lines: result
            .lines
            .into_iter()
            .map(|line| CalorieLine {
                index: line.index,
                item: line.item,
                calories: line.calories.map(Into::into),
                text: line.text,
            })
            .collect(),
    }))
}
