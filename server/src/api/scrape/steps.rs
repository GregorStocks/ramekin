use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::schema::{scrape_jobs, step_outputs};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use uuid::Uuid;

#[utoipa::path(
    get,
    path = "/api/scrape/{id}/steps/{step_name}/output",
    tag = "scrape",
    params(
        ("id" = Uuid, Path, description = "Scrape job ID"),
        ("step_name" = String, Path, description = "Pipeline step name"),
    ),
    responses(
        (status = 200, description = "Raw step output JSON", body = serde_json::Value),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Step output not found", body = ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_step_output(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path((job_id, step_name)): Path<(Uuid, String)>,
) -> impl IntoResponse {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to get DB connection: {}", e);
            return ApiError::internal("Database error").into_response();
        }
    };

    let owner: Option<Uuid> = match scrape_jobs::table
        .filter(scrape_jobs::id.eq(job_id))
        .select(scrape_jobs::user_id)
        .first::<Uuid>(&mut conn)
        .optional()
    {
        Ok(o) => o,
        Err(e) => {
            tracing::error!("Failed to look up scrape job owner: {}", e);
            return ApiError::internal("Database error").into_response();
        }
    };

    match owner {
        Some(uid) if uid == user.id => {}
        _ => {
            return ApiError::not_found("Step output not found").into_response();
        }
    }

    let output: Option<JsonValue> = match step_outputs::table
        .filter(step_outputs::scrape_job_id.eq(job_id))
        .filter(step_outputs::step_name.eq(&step_name))
        .order(step_outputs::created_at.desc())
        .select(step_outputs::output)
        .first::<JsonValue>(&mut conn)
        .optional()
    {
        Ok(o) => o,
        Err(e) => {
            tracing::error!("Failed to load step output: {}", e);
            return ApiError::internal("Database error").into_response();
        }
    };

    match output {
        Some(o) => (StatusCode::OK, Json(o)).into_response(),
        None => ApiError::not_found("Step output not found").into_response(),
    }
}
