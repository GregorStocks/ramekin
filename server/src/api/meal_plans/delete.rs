use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::schema::meal_plans;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use diesel::prelude::*;
use std::sync::Arc;
use uuid::Uuid;

#[utoipa::path(
    delete,
    path = "/api/meal-plans/{id}",
    tag = "meal_plans",
    params(
        ("id" = Uuid, Path, description = "Meal plan ID")
    ),
    responses(
        (status = 204, description = "Meal plan deleted"),
        (status = 404, description = "Meal plan not found", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_meal_plan(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;

    let updated = run_db(&pool, move |conn| {
        // Soft delete - set deleted_at timestamp
        diesel::update(
            meal_plans::table
                .filter(meal_plans::id.eq(id))
                .filter(meal_plans::user_id.eq(user_id))
                .filter(meal_plans::deleted_at.is_null()),
        )
        .set(meal_plans::deleted_at.eq(Some(Utc::now())))
        .execute(conn)
        .map_err(|e| {
            tracing::error!("Failed to delete meal plan: {}", e);
            ApiError::internal("Failed to delete meal plan")
        })
    })
    .await?;

    if updated == 0 {
        return Err(ApiError::not_found("Meal plan not found"));
    }

    Ok(StatusCode::NO_CONTENT)
}
