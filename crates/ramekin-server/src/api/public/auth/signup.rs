use crate::api::ErrorResponse;
use crate::auth::{create_session, hash_password};
use crate::db::DbPool;
use crate::models::NewUser;
use crate::schema::users;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use serde::Deserialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

pub const PATH: &str = "/api/auth/signup";

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SignupRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, serde::Serialize, ToSchema)]
pub struct SignupResponse {
    pub user_id: Uuid,
    pub token: String,
}

#[utoipa::path(
    post,
    path = "/api/auth/signup",
    tag = "auth",
    request_body(content = SignupRequest, example = json!({"username": "user", "password": "password"})),
    responses(
        (status = 201, description = "User created successfully", body = SignupResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 409, description = "Username already exists", body = ErrorResponse)
    )
)]
pub async fn signup(
    State(pool): State<Arc<DbPool>>,
    Json(req): Json<SignupRequest>,
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

    let password_hash = match hash_password(&req.password) {
        Ok(h) => h,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to hash password".to_string(),
                }),
            )
                .into_response()
        }
    };

    let new_user = NewUser {
        username: &req.username,
        password_hash: &password_hash,
    };

    let user: crate::models::User = match diesel::insert_into(users::table)
        .values(&new_user)
        .returning(crate::models::User::as_returning())
        .get_result(&mut conn)
    {
        Ok(u) => u,
        Err(diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::UniqueViolation,
            _,
        )) => {
            return (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: "Username already exists".to_string(),
                }),
            )
                .into_response()
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to create user".to_string(),
                }),
            )
                .into_response()
        }
    };

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

    (
        StatusCode::CREATED,
        Json(SignupResponse {
            user_id: user.id,
            token,
        }),
    )
        .into_response()
}
