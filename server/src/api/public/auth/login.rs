use crate::api::ErrorResponse;
use crate::auth::{create_session_with_token, verify_password, DEV_TEST_TOKEN};
use crate::db::DbPool;
use crate::get_conn;
use crate::models::User;
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
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

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

    // For test user "t", return the fixed token (session already exists from signup)
    let token = if user.username.to_lowercase() == "t" {
        DEV_TEST_TOKEN.to_string()
    } else {
        match create_session_with_token(&mut conn, user.id, None) {
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
        }
    };

    (StatusCode::OK, Json(LoginResponse { token })).into_response()
}
