use crate::api::{ApiError, ErrorResponse};
use crate::auth::{create_session_with_token, verify_password, DEV_TEST_TOKEN};
use crate::db::DbPool;
use crate::get_conn;
use crate::models::User;
use crate::schema::users;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;

// SQL function declaration for PostgreSQL LOWER() on text
diesel::define_sql_function! {
    fn lower(x: diesel::sql_types::Text) -> diesel::sql_types::Text;
}
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
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    let user: User = match users::table
        .filter(lower(users::username).eq(req.username.to_lowercase()))
        .filter(users::deleted_at.is_null())
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(_) => return ApiError::unauthorized("Invalid credentials").into_response(),
    };

    if !verify_password(&req.password, &user.password_hash) {
        return ApiError::unauthorized("Invalid credentials").into_response();
    }

    // For test user "t", use the fixed dev token so it's predictable
    let fixed_token = if user.username.to_lowercase() == "t" {
        Some(DEV_TEST_TOKEN)
    } else {
        None
    };
    let token = match create_session_with_token(&mut conn, user.id, fixed_token) {
        Ok(t) => t,
        Err(_) => return ApiError::internal("Failed to create session").into_response(),
    };

    (StatusCode::OK, Json(LoginResponse { token })).into_response()
}
