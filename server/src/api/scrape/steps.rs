use crate::api::{run_db, ApiError, ErrorResponse};
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
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;
    let output = run_db(&pool, move |conn| {
        let owner: Option<Uuid> = scrape_jobs::table
            .filter(scrape_jobs::id.eq(job_id))
            .select(scrape_jobs::user_id)
            .first::<Uuid>(conn)
            .optional()
            .map_err(|e| {
                tracing::error!("Failed to look up scrape job owner: {}", e);
                ApiError::internal("Database error")
            })?;

        match owner {
            Some(uid) if uid == user_id => {}
            _ => {
                return Err(ApiError::not_found("Step output not found"));
            }
        }

        let output: Option<JsonValue> = step_outputs::table
            .filter(step_outputs::scrape_job_id.eq(job_id))
            .filter(step_outputs::step_name.eq(&step_name))
            .order(step_outputs::created_at.desc())
            .select(step_outputs::output)
            .first::<JsonValue>(conn)
            .optional()
            .map_err(|e| {
                tracing::error!("Failed to load step output: {}", e);
                ApiError::internal("Database error")
            })?;

        output.ok_or_else(|| ApiError::not_found("Step output not found"))
    })
    .await?;

    Ok((StatusCode::OK, Json(output)))
}
