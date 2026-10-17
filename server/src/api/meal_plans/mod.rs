pub mod list;

use crate::AppState;
use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for /api/meal-plans endpoints (mounted at /api/meal-plans).
/// Writes go through `POST /api/sync/meal-plans`.
pub fn router() -> Router<AppState> {
    Router::new().route("/", get(list::list_meal_plans))
}

#[derive(OpenApi)]
#[openapi(
    paths(list::list_meal_plans),
    components(schemas(list::MealPlanListResponse, list::MealPlanItem, list::MealType,))
)]
pub struct ApiDoc;
