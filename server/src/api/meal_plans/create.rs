use super::list::MealType;
use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::NewMealPlan;
use crate::schema::{meal_plans, recipes};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::NaiveDate;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateMealPlanRequest {
    pub recipe_id: Uuid,
    pub meal_date: NaiveDate,
    pub meal_type: MealType,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CreateMealPlanResponse {
    pub id: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/meal-plans",
    tag = "meal_plans",
    request_body = CreateMealPlanRequest,
    responses(
        (status = 201, description = "Meal plan created", body = CreateMealPlanResponse),
        (status = 400, description = "Invalid request (recipe not found or deleted)", body = ErrorResponse),
        (status = 409, description = "Duplicate meal plan entry", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_meal_plan(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<CreateMealPlanRequest>,
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    // Verify recipe exists and belongs to user
    let recipe_exists: bool = match recipes::table
        .filter(recipes::id.eq(request.recipe_id))
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .select(recipes::id)
        .first::<Uuid>(&mut conn)
        .optional()
    {
        Ok(record) => record.is_some(),
        Err(e) => {
            tracing::error!("Failed to verify recipe ownership: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to verify recipe".to_string(),
                }),
            )
                .into_response();
        }
    };

    if !recipe_exists {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Recipe not found".to_string(),
            }),
        )
            .into_response();
    }

    // Insert the meal plan
    let notes_ref = request.notes.as_deref();
    let result: Result<Uuid, DieselError> = diesel::insert_into(meal_plans::table)
        .values(NewMealPlan {
            user_id: user.id,
            recipe_id: request.recipe_id,
            meal_date: request.meal_date,
            meal_type: request.meal_type.as_str(),
            notes: notes_ref,
        })
        .returning(meal_plans::id)
        .get_result(&mut conn);

    match result {
        Ok(id) => (StatusCode::CREATED, Json(CreateMealPlanResponse { id })).into_response(),
        Err(DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "This recipe is already planned for this meal".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to create meal plan: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to create meal plan".to_string(),
                }),
            )
                .into_response()
        }
    }
}
