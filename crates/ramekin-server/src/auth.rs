use crate::api::{
    ErrorResponse, HelloResponse, LoginRequest, LoginResponse, SignupRequest, SignupResponse,
};
use crate::db::DbPool;
use crate::models::{NewSession, NewUser, Session, User};
use crate::schema::{sessions, users};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{Duration, Utc};
use diesel::prelude::*;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::sync::Arc;

fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

fn verify_password(password: &str, hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

fn create_session(
    conn: &mut PgConnection,
    user_id: uuid::Uuid,
) -> Result<String, diesel::result::Error> {
    let token = generate_token();
    let token_hash = hash_token(&token);
    let expires_at = Utc::now() + Duration::days(30);

    let new_session = NewSession {
        user_id,
        token_hash: &token_hash,
        expires_at,
    };

    diesel::insert_into(sessions::table)
        .values(&new_session)
        .execute(conn)?;

    Ok(token)
}

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

    // Hash the password
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

    // Insert user
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

    // Create session
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

    // Find user by username (case-insensitive, not deleted)
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

    // Verify password
    if !verify_password(&req.password, &user.password_hash) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Invalid credentials".to_string(),
            }),
        )
            .into_response();
    }

    // Create session
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

pub async fn get_user_from_token(pool: &DbPool, token: &str) -> Option<User> {
    let mut conn = pool.get().ok()?;
    let token_hash = hash_token(token);

    // Find session by token hash, not expired
    let session: Session = sessions::table
        .filter(sessions::token_hash.eq(&token_hash))
        .filter(sessions::expires_at.gt(Utc::now()))
        .select(Session::as_select())
        .first(&mut conn)
        .ok()?;

    // Find user, not deleted
    users::table
        .filter(users::id.eq(session.user_id))
        .filter(users::deleted_at.is_null())
        .select(User::as_select())
        .first(&mut conn)
        .ok()
}

#[utoipa::path(
    get,
    path = "/api/auth/hello",
    responses(
        (status = 200, description = "Hello message", body = HelloResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn hello(
    State(pool): State<Arc<DbPool>>,
    headers: header::HeaderMap,
) -> impl IntoResponse {
    // Extract Bearer token
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

    // Validate token and get user
    let user = match get_user_from_token(&pool, token).await {
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
        Json(HelloResponse {
            message: format!("Hello, {}!", user.username),
        }),
    )
        .into_response()
}
