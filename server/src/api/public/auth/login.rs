use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::{create_session_with_token, verify_password, DEV_TEST_TOKEN};
use crate::db::DbPool;
use crate::models::User;
use crate::raw_sql;
use crate::schema::users;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LoginResponse {
    pub token: String,
}

#[utoipa::path(
    post,
    path = "/api/auth/login",
    tag = "auth",
    request_body(content = LoginRequest, example = json!({"username": "user", "password": "password"})),
    responses(
        (status = 200, description = "Login successful", body = LoginResponse),
        (status = 401, description = "Invalid credentials", body = ErrorResponse)
    )
)]
pub async fn login(
    State(pool): State<Arc<DbPool>>,
    Json(req): Json<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let token = run_db(&pool, move |conn| {
        let user: User = users::table
            .filter(raw_sql::lower(users::username).eq(req.username.to_lowercase()))
            .filter(users::deleted_at.is_null())
            .select(User::as_select())
            .first(conn)
            .map_err(|_| ApiError::unauthorized("Invalid credentials"))?;

        if !verify_password(&req.password, &user.password_hash) {
            return Err(ApiError::unauthorized("Invalid credentials"));
        }

        // For test user "t", use the fixed dev token so it's predictable
        let fixed_token = if user.username.to_lowercase() == "t" {
            Some(DEV_TEST_TOKEN)
        } else {
            None
        };
        create_session_with_token(conn, user.id, fixed_token)
            .map_err(|_| ApiError::internal("Failed to create session"))
    })
    .await?;

    Ok((StatusCode::OK, Json(LoginResponse { token })))
}
