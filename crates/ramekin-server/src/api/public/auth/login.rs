use crate::api::ErrorResponse;
use crate::auth::{create_session, verify_password};
use crate::db::DbPool;
use crate::models::User;
use crate::schema::users;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

pub const PATH: &str = "/api/auth/login";

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
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Database connection failed".to_string(),
                }),
            )
                .into_response()
        }
    };

    let user: User = match users::table
        .filter(
            diesel::dsl::sql::<diesel::sql_types::Bool>("LOWER(username) = LOWER(")
                .bind::<diesel::sql_types::Text, _>(&req.username)
                .sql(")"),
        )
        .filter(users::deleted_at.is_null())
        .select(User::as_select())
        .first(&mut conn)
    {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Invalid credentials".to_string(),
                }),
            )
                .into_response()
        }
    };

    if !verify_password(&req.password, &user.password_hash) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Invalid credentials".to_string(),
            }),
        )
            .into_response();
    }

    let token = match create_session(&mut conn, user.id) {
        Ok(t) => t,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to create session".to_string(),
                }),
            )
                .into_response()
        }
    };

    (StatusCode::OK, Json(LoginResponse { token })).into_response()
}
