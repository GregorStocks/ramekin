use super::list::MealType;
use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::schema::meal_plans;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use serde::Deserialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, ToSchema, Default)]
pub struct UpdateMealPlanRequest {
    pub meal_date: Option<NaiveDate>,
    pub meal_type: Option<MealType>,
    /// Set to empty string to clear notes, or provide new value
    pub notes: Option<String>,
}

#[utoipa::path(
    put,
    path = "/api/meal-plans/{id}",
    tag = "meal_plans",
    params(
        ("id" = Uuid, Path, description = "Meal plan ID")
    ),
    request_body = UpdateMealPlanRequest,
    responses(
        (status = 200, description = "Meal plan updated"),
        (status = 404, description = "Meal plan not found", body = ErrorResponse),
        (status = 409, description = "Conflict with existing meal plan", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_meal_plan(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateMealPlanRequest>,
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    // Fetch the existing meal plan
    let existing: Option<(NaiveDate, String, Option<String>)> = meal_plans::table
        .filter(meal_plans::id.eq(id))
        .filter(meal_plans::user_id.eq(user.id))
        .filter(meal_plans::deleted_at.is_null())
        .select((
            meal_plans::meal_date,
            meal_plans::meal_type,
            meal_plans::notes,
        ))
        .first(&mut conn)
        .optional()
        .unwrap_or(None);

    let Some((current_date, current_type, current_notes)) = existing else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Meal plan not found".to_string(),
            }),
        )
            .into_response();
    };

    // Calculate new values
    let new_date = request.meal_date.unwrap_or(current_date);
    let new_type = request
        .meal_type
        .map(|mt| mt.as_str().to_string())
        .unwrap_or(current_type);
    let new_notes = match &request.notes {
        Some(n) if n.is_empty() => None,
        Some(n) => Some(n.clone()),
        None => current_notes,
    };

    // Update the meal plan
    let result = diesel::update(
        meal_plans::table
            .filter(meal_plans::id.eq(id))
            .filter(meal_plans::user_id.eq(user.id))
            .filter(meal_plans::deleted_at.is_null()),
    )
    .set((
        meal_plans::meal_date.eq(new_date),
        meal_plans::meal_type.eq(&new_type),
        meal_plans::notes.eq(&new_notes),
    ))
    .execute(&mut conn);

    match result {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Meal plan not found".to_string(),
            }),
        )
            .into_response(),
        Ok(_) => StatusCode::OK.into_response(),
        Err(DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "This recipe is already planned for this meal".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to update meal plan: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to update meal plan".to_string(),
                }),
            )
                .into_response()
        }
    }
}
