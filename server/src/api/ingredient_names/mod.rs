//! Status and controls for resolving ingredient names the catalog doesn't
//! know (catalog step 3). The resolution table is shared across accounts.

use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::ingredient_names::{enqueue, unlearned_names, wake, FAILED, PENDING, RESOLVED};
use crate::models::Ingredient;
use crate::schema::{
    ingredient_name_resolutions as names, recipe_versions, recipes, shopping_list_items,
};
use crate::AppState;
use axum::routing::{get, post};
use axum::{extract::State, Json, Router};
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use utoipa::{OpenApi, ToSchema};

/// Failed names shown in the status.
const FAILURES_SHOWN: i64 = 20;

#[derive(Debug, Serialize, ToSchema)]
pub struct IngredientNameFailure {
    pub name: String,
    pub error: String,
    pub attempts: i32,
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
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IngredientNamesQueuedResponse {
    /// Names queued for resolution by this call.
    pub queued: usize,
}

fn db_error(e: diesel::result::Error) -> ApiError {
    tracing::error!("Ingredient name resolution query failed: {}", e);
    ApiError::internal("Failed to read ingredient names")
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
    AuthUser(_user): AuthUser,
    State(pool): State<Arc<DbPool>>,
) -> Result<Json<IngredientNamesStatusResponse>, ApiError> {
    let response = run_db(&pool, |conn| {
        let counts: Vec<(String, Option<String>, i64)> = names::table
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
            .filter(names::status.eq(FAILED))
            .order(names::updated_at.desc())
            .limit(FAILURES_SHOWN)
            .select((names::name, names::error, names::attempts))
            .load(conn)
            .map_err(db_error)?;
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
        (status = 200, description = "Failed names queued again", body = IngredientNamesQueuedResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn retry_ingredient_names(
    AuthUser(_user): AuthUser,
    State(pool): State<Arc<DbPool>>,
) -> Result<Json<IngredientNamesQueuedResponse>, ApiError> {
    let queued = run_db(&pool, |conn| {
        diesel::update(names::table.filter(names::status.eq(FAILED)))
            .set((names::status.eq(PENDING), names::error.eq(None::<String>)))
            .execute(conn)
            .map_err(db_error)
    })
    .await?;
    wake();
    Ok(Json(IngredientNamesQueuedResponse { queued }))
}

#[utoipa::path(
    post,
    path = "/api/ingredient-names/warm",
    tag = "ingredient_names",
    responses(
        (status = 200, description = "Unknown names in the caller's recipes and shopping list queued", body = IngredientNamesQueuedResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
/// Queue every name the catalog doesn't know from the caller's current
/// recipes and shopping list, e.g. once after this feature ships.
pub async fn warm_ingredient_names(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
) -> Result<Json<IngredientNamesQueuedResponse>, ApiError> {
    let user_id = user.id;
    let queued = run_db(&pool, move |conn| {
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
        let mut items: Vec<String> = Vec::new();
        for ingredients in versions {
            let ingredients: Vec<Ingredient> = serde_json::from_value(ingredients)
                .map_err(|_| ApiError::internal("Stored ingredients are not valid"))?;
            items.extend(ingredients.into_iter().map(|i| i.item));
        }
        let shopping: Vec<String> = shopping_list_items::table
            .filter(shopping_list_items::user_id.eq(user_id))
            .filter(shopping_list_items::deleted_at.is_null())
            .select(shopping_list_items::item)
            .load(conn)
            .map_err(db_error)?;
        items.extend(shopping);
        let pending = unlearned_names(items.iter().map(String::as_str));
        enqueue(conn, &pending).map_err(db_error)
    })
    .await?;
    wake();
    Ok(Json(IngredientNamesQueuedResponse { queued }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/status", get(get_ingredient_names_status))
        .route("/retry", post(retry_ingredient_names))
        .route("/warm", post(warm_ingredient_names))
}

#[derive(OpenApi)]
#[openapi(
    paths(
        get_ingredient_names_status,
        retry_ingredient_names,
        warm_ingredient_names
    ),
    components(schemas(
        IngredientNamesStatusResponse,
        IngredientNameFailure,
        IngredientNamesQueuedResponse
    ))
)]
pub struct ApiDoc;
