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
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    // Soft delete - set deleted_at timestamp
    let updated = match diesel::update(
        meal_plans::table
            .filter(meal_plans::id.eq(id))
            .filter(meal_plans::user_id.eq(user.id))
            .filter(meal_plans::deleted_at.is_null()),
    )
    .set(meal_plans::deleted_at.eq(Some(Utc::now())))
    .execute(&mut conn)
    {
        Ok(count) => count,
        Err(e) => {
            tracing::error!("Failed to delete meal plan: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to delete meal plan".to_string(),
                }),
            )
                .into_response();
        }
    };

    if updated == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Meal plan not found".to_string(),
            }),
        )
            .into_response();
    }

    StatusCode::NO_CONTENT.into_response()
}
