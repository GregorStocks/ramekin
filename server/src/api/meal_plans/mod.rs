pub mod create;
pub mod delete;
pub mod list;
pub mod update;

use crate::AppState;
use axum::routing::{get, put};
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for /api/meal-plans endpoints (mounted at /api/meal-plans)
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/",
            get(list::list_meal_plans).post(create::create_meal_plan),
        )
        .route(
            "/{id}",
            put(update::update_meal_plan).delete(delete::delete_meal_plan),
        )
}

#[derive(OpenApi)]
#[openapi(
    paths(
        list::list_meal_plans,
        create::create_meal_plan,
        update::update_meal_plan,
        delete::delete_meal_plan
    ),
    components(schemas(
        list::MealPlanListResponse,
        list::MealPlanItem,
        list::MealType,
        create::CreateMealPlanRequest,
        create::CreateMealPlanResponse,
        update::UpdateMealPlanRequest,
    ))
)]
pub struct ApiDoc;
