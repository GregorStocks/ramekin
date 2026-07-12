use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::{create_session_with_token, hash_password, DEV_TEST_TOKEN};
use crate::db::DbPool;
use crate::models::NewUser;
use crate::schema::users;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use serde::Deserialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

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
) -> Result<impl IntoResponse, ApiError> {
    let (user_id, token) = run_db(&pool, move |conn| {
        let password_hash = hash_password(&req.password)
            .map_err(|_| ApiError::internal("Failed to hash password"))?;

        let new_user = NewUser {
            username: &req.username,
            password_hash: &password_hash,
        };

        let user: crate::models::User = match diesel::insert_into(users::table)
            .values(&new_user)
            .returning(crate::models::User::as_returning())
            .get_result(conn)
        {
            Ok(u) => u,
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            )) => return Err(ApiError::conflict("Username already exists")),
            Err(_) => return Err(ApiError::internal("Failed to create user")),
        };

        // Use fixed token for test user "t" so session persists across DB resets
        let fixed_token = if req.username.to_lowercase() == "t" {
            Some(DEV_TEST_TOKEN)
        } else {
            None
        };
        let token = create_session_with_token(conn, user.id, fixed_token)
            .map_err(|_| ApiError::internal("Failed to create session"))?;

        Ok((user.id, token))
    })
    .await?;

    Ok((StatusCode::CREATED, Json(SignupResponse { user_id, token })))
}
