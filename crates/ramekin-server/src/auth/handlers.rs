use crate::api::{
    ErrorResponse, LoginRequest, LoginResponse, PingResponse, SignupRequest, SignupResponse,
};
use crate::db::DbPool;
use crate::models::{NewUser, User};
use crate::schema::users;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use std::sync::Arc;

use super::crypto::{hash_password, verify_password};
use super::db::{create_session, get_user_from_token};

#[utoipa::path(
    post,
    path = "/api/auth/signup",
    request_body = SignupRequest,
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

    let user: User = match diesel::insert_into(users::table)
        .values(&new_user)
        .returning(User::as_returning())
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

#[utoipa::path(
    post,
    path = "/api/auth/login",
    request_body = LoginRequest,
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

#[utoipa::path(
    get,
    path = "/api/test/ping",
    responses(
        (status = 200, description = "Authenticated ping response", body = PingResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn ping(
    State(pool): State<Arc<DbPool>>,
    headers: header::HeaderMap,
) -> impl IntoResponse {
    let auth_header = match headers.get(header::AUTHORIZATION) {
        Some(h) => h,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Missing Authorization header".to_string(),
                }),
            )
                .into_response()
        }
    };

    let auth_str = match auth_header.to_str() {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Invalid Authorization header".to_string(),
                }),
            )
                .into_response()
        }
    };

    let token = match auth_str.strip_prefix("Bearer ") {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Invalid Authorization header format".to_string(),
                }),
            )
                .into_response()
        }
    };

    let _user = match get_user_from_token(&pool, token).await {
        Some(u) => u,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Invalid or expired token".to_string(),
                }),
            )
                .into_response()
        }
    };

    (
        StatusCode::OK,
        Json(PingResponse {
            message: "ping".to_string(),
        }),
    )
        .into_response()
}
