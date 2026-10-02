//! Status and controls for resolving ingredient names the catalog doesn't
//! know, and estimating weights it lacks (catalog step 3). The resolution table is shared across accounts, but
//! each account only sees and retries the names in its own recipes and
//! shopping list.

use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::ingredient_names::{
    load_learned, requeue_failed, unlearned_names, wake, FAILED, PENDING, RESOLVED,
};
use crate::ingredient_weights;
use crate::models::Ingredient;
use crate::schema::{
    ingredient_name_resolutions as names, recipe_versions, recipes, shopping_list_items,
};
use crate::AppState;
use axum::routing::{get, post};
use axum::{extract::State, Json, Router};
use diesel::prelude::*;
use ramekin_core::ingredient_parser::ParsedIngredient;
use ramekin_core::nutrition::{self, WeightKey, Weights};
use serde::Serialize;
use std::collections::BTreeSet;
use std::sync::Arc;
use utoipa::{OpenApi, ToSchema};
use uuid::Uuid;

/// Failed names shown in the status.
const FAILURES_SHOWN: i64 = 20;

#[derive(Debug, Serialize, ToSchema)]
pub struct IngredientNameFailure {
    pub name: String,
    pub error: String,
    pub attempts: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IngredientWeightFailure {
    /// The catalog food, as the catalog names it.
    pub food: String,
    pub unit: String,
    pub error: String,
    pub attempts: i32,
}

/// Weights the catalog lacks for foods in the user's recipes (a density, or
/// a counted unit such as "bunch"), estimated in the background.
#[derive(Debug, Serialize, ToSchema)]
pub struct IngredientWeightsStatus {
    /// Estimated; lines using one say "estimated weight".
    pub estimated: i64,
    /// The model said there's no typical weight; still unknown in estimates.
    pub no_typical_weight: i64,
    /// Waiting for the background estimator (queued when an estimate is shown).
    pub pending: i64,
    /// The last attempt failed; retry to try again.
    pub failed: i64,
    /// The most recent failures.
    pub failures: Vec<IngredientWeightFailure>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IngredientNamesStatusResponse {
    /// Resolved to a catalog food or product.
    pub recognized: i64,
    /// Resolved as not an ingredient (a heading, a serving note).
    pub not_food: i64,
    /// Resolved, but the model couldn't tell; still unknown in estimates.
    pub unknown: i64,
    /// Waiting for the background resolver.
    pub pending: i64,
    /// The last attempt failed; retry to try again.
    pub failed: i64,
    /// The most recent failures.
    pub failures: Vec<IngredientNameFailure>,
    pub weights: IngredientWeightsStatus,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IngredientNamesQueuedResponse {
    /// Names and weights queued again by this call.
    pub queued: usize,
}

fn db_error(e: diesel::result::Error) -> ApiError {
    tracing::error!("Ingredient name resolution query failed: {}", e);
    ApiError::internal("Failed to read ingredient names")
}

/// The user's current recipes' ingredients, one list per recipe.
fn own_recipes(conn: &mut PgConnection, user_id: Uuid) -> Result<Vec<Vec<Ingredient>>, ApiError> {
    let versions: Vec<serde_json::Value> = recipes::table
        .inner_join(
            recipe_versions::table
                .on(recipes::current_version_id.eq(recipe_versions::id.nullable())),
        )
        .filter(recipes::user_id.eq(user_id))
        .filter(recipes::deleted_at.is_null())
        .select(recipe_versions::ingredients)
        .load(conn)
        .map_err(db_error)?;
    versions
        .into_iter()
        .map(|ingredients| {
            serde_json::from_value(ingredients)
                .map_err(|_| ApiError::internal("Stored ingredients are not valid"))
        })
        .collect()
}

/// The names the catalog doesn't know in the user's current recipes and
/// shopping list.
fn own_names(
    conn: &mut PgConnection,
    user_id: Uuid,
    recipes: &[Vec<Ingredient>],
) -> Result<Vec<String>, ApiError> {
    let mut items: Vec<String> = recipes
        .iter()
        .flatten()
        .map(|ingredient| ingredient.item.clone())
        .collect();
    let shopping: Vec<String> = shopping_list_items::table
        .filter(shopping_list_items::user_id.eq(user_id))
        .filter(shopping_list_items::deleted_at.is_null())
        .select(shopping_list_items::item)
        .load(conn)
        .map_err(db_error)?;
    items.extend(shopping);
    Ok(unlearned_names(items.iter().map(String::as_str)))
}

/// The weights the catalog lacks for the user's current recipes, as their
/// calorie estimates report them.
fn own_weight_gaps(
    conn: &mut PgConnection,
    recipes: Vec<Vec<Ingredient>>,
) -> Result<Vec<WeightKey>, ApiError> {
    let learned = load_learned(
        conn,
        recipes
            .iter()
            .flatten()
            .map(|ingredient| ingredient.item.as_str()),
    )
    .map_err(db_error)?;
    let mut gaps = BTreeSet::new();
    for ingredients in recipes {
        let parsed: Vec<ParsedIngredient> = ingredients.into_iter().map(Into::into).collect();
        let estimate = nutrition::estimate_with(&parsed, None, 1.0, &learned, &Weights::new())
            .map_err(|e| ApiError::internal(format!("Failed to estimate stored recipe: {e}")))?;
        gaps.extend(estimate.weight_gaps);
    }
    Ok(gaps.into_iter().collect())
}

#[utoipa::path(
    get,
    path = "/api/ingredient-names/status",
    tag = "ingredient_names",
    responses(
        (status = 200, description = "Resolution status of ingredient names the catalog doesn't know", body = IngredientNamesStatusResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_ingredient_names_status(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
) -> Result<Json<IngredientNamesStatusResponse>, ApiError> {
    let user_id = user.id;
    let response = run_db(&pool, move |conn| {
        let recipes = own_recipes(conn, user_id)?;
        let own = own_names(conn, user_id, &recipes)?;
        let counts: Vec<(String, Option<String>, i64)> = names::table
            .filter(names::name.eq_any(&own))
            .group_by((names::status, names::disposition))
            .select((names::status, names::disposition, diesel::dsl::count_star()))
            .load(conn)
            .map_err(db_error)?;
        let count = |status: &str, disposition: Option<&str>| {
            counts
                .iter()
                .filter(|(s, d, _)| {
                    s == status && (disposition.is_none() || d.as_deref() == disposition)
                })
                .map(|(_, _, n)| n)
                .sum::<i64>()
        };
        let failures: Vec<(String, Option<String>, i32)> = names::table
            .filter(names::name.eq_any(&own))
            .filter(names::status.eq(FAILED))
            .order(names::updated_at.desc())
            .limit(FAILURES_SHOWN)
            .select((names::name, names::error, names::attempts))
            .load(conn)
            .map_err(db_error)?;
        let gaps = own_weight_gaps(conn, recipes)?;
        let weights = ingredient_weights::counts(conn, &gaps).map_err(db_error)?;
        Ok(IngredientNamesStatusResponse {
            recognized: count(RESOLVED, Some("entry")),
            not_food: count(RESOLVED, Some("not_food")),
            unknown: count(RESOLVED, Some("unknown")),
            pending: count(PENDING, None),
            failed: count(FAILED, None),
            failures: failures
                .into_iter()
                .map(|(name, error, attempts)| IngredientNameFailure {
                    name,
                    error: error.unwrap_or_default(),
                    attempts,
                })
                .collect(),
            weights: IngredientWeightsStatus {
                estimated: weights.estimated,
                no_typical_weight: weights.no_typical_weight,
                pending: weights.pending,
                failed: weights.failed.len() as i64,
                failures: weights
                    .failed
                    .into_iter()
                    .take(FAILURES_SHOWN as usize)
                    .map(|(key, error, attempts)| IngredientWeightFailure {
                        food: key.food,
                        unit: key.unit,
                        error,
                        attempts,
                    })
                    .collect(),
            },
        })
    })
    .await?;
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/ingredient-names/retry",
    tag = "ingredient_names",
    responses(
        (status = 200, description = "Failed names and weights queued again", body = IngredientNamesQueuedResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn retry_ingredient_names(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
) -> Result<Json<IngredientNamesQueuedResponse>, ApiError> {
    let user_id = user.id;
    let queued = run_db(&pool, move |conn| {
        let recipes = own_recipes(conn, user_id)?;
        let own = own_names(conn, user_id, &recipes)?;
        let names = requeue_failed(conn, &own).map_err(db_error)?;
        let gaps = own_weight_gaps(conn, recipes)?;
        let weights = ingredient_weights::requeue_failed(conn, &gaps).map_err(db_error)?;
        Ok(names + weights)
    })
    .await?;
    wake();
    Ok(Json(IngredientNamesQueuedResponse { queued }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/status", get(get_ingredient_names_status))
        .route("/retry", post(retry_ingredient_names))
}

#[derive(OpenApi)]
#[openapi(
    paths(get_ingredient_names_status, retry_ingredient_names),
    components(schemas(
        IngredientNamesStatusResponse,
        IngredientNameFailure,
        IngredientWeightsStatus,
        IngredientWeightFailure,
        IngredientNamesQueuedResponse
    ))
)]
pub struct ApiDoc;
