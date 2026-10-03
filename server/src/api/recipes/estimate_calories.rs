use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::Ingredient;
use axum::{extract::State, Json};
use ramekin_core::ingredient_parser::ParsedIngredient;
use ramekin_core::nutrition;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
pub struct EstimateCaloriesRequest {
    pub ingredients: Vec<Ingredient>,
    /// Original, unscaled servings text: a count or range, optionally with a
    /// "serves", "servings", "yield", or "makes" prefix and a "servings",
    /// "people", "person(s)", or "portion(s)" suffix ("Serves 4 to 6", "Yield:
    /// 4"). A "makes" count needs a serving suffix; a yield of something else
    /// ("Makes 12 cookies", "Makes 24") gives no per-serving figure.
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
    /// Display text: "~120 kcal", "Negligible", "Not a food", "No amount
    /// given", or why it couldn't be counted ("Not recognized", "Amount
    /// unclear").
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
    /// Ingredients the figures leave out, in recipe order: those given no
    /// amount (even in a complete estimate), and for a partial estimate the
    /// ones that couldn't be counted.
    pub not_counted: Vec<String>,
    /// Null when nothing was counted. A lower bound when status is partial.
    pub known_calories: Option<CalorieRange>,
    pub per_serving_calories: Option<CalorieRange>,
    /// The breakdown, one entry per ingredient in order.
    pub lines: Vec<CalorieLine>,
    /// Some ingredient names are still being recognized, or weights
    /// estimated, in the background; ask again shortly for an estimate that
    /// includes them.
    pub resolving: bool,
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
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<EstimateCaloriesRequest>,
) -> Result<Json<CalorieEstimateResponse>, ApiError> {
    let ingredients: Vec<ParsedIngredient> =
        request.ingredients.into_iter().map(Into::into).collect();
    // Stored answers for names the committed catalog doesn't know; names not
    // resolved yet read as unknown.
    let items: Vec<String> = ingredients.iter().map(|i| i.item.clone()).collect();
    let (learned, resolving_names) = run_db(&pool, move |conn| {
        let names = || items.iter().map(String::as_str);
        // Pending first: a name leaves pending in the same update that stores
        // its answer, so once nothing reads as pending every answer is
        // committed and the load below sees it. Loading first could miss an
        // answer that lands between the two reads and then report nothing
        // pending, ending the client's polling on a stale "Not recognized".
        crate::ingredient_names::any_pending(conn, names())
            .and_then(|resolving| {
                crate::ingredient_names::load_learned(conn, names())
                    .map(|learned| (learned, resolving))
            })
            .map_err(|e| {
                tracing::error!("Failed to load learned ingredient names: {}", e);
                ApiError::internal("Failed to load ingredient names")
            })
    })
    .await?;
    let estimate = |weights: &nutrition::Weights| {
        nutrition::estimate_with(
            &ingredients,
            request.servings.as_deref(),
            request.scale,
            &learned,
            weights,
        )
        .map_err(ApiError::invalid_request)
    };
    // Weights the catalog lacks for foods it knows: estimated in the
    // background once reported, then read back here. Queuing a gap is the
    // only write; the model is never called on this path.
    let gaps = estimate(&nutrition::Weights::new())?.weight_gaps;
    let (weights, resolving_weights) = run_db(&pool, move |conn| {
        crate::ingredient_weights::load_and_enqueue(conn, &gaps).map_err(|e| {
            tracing::error!("Failed to load estimated ingredient weights: {}", e);
            ApiError::internal("Failed to load ingredient weights")
        })
    })
    .await?;
    if resolving_weights {
        crate::ingredient_names::wake();
    }
    let result = estimate(&weights)?;
    let resolving = resolving_names || resolving_weights;
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
        resolving,
    }))
}
