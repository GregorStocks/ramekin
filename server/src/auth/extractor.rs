use crate::api::ErrorResponse;
use crate::db::DbPool;
use crate::models::User;
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::sync::Arc;

use super::db::get_user_from_token;

/// Extractor that validates the Authorization header and provides the authenticated user.
///
/// Use this in any handler that requires authentication:
/// ```ignore
/// async fn my_handler(user: AuthUser) -> impl IntoResponse {
///     // user.0 is the authenticated User
/// }
/// ```
pub struct AuthUser(pub User);

pub enum AuthError {
    MissingHeader,
    InvalidHeader,
    InvalidFormat,
    InvalidToken,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingHeader => (StatusCode::UNAUTHORIZED, "Missing Authorization header"),
            AuthError::InvalidHeader => (StatusCode::UNAUTHORIZED, "Invalid Authorization header"),
            AuthError::InvalidFormat => (
                StatusCode::UNAUTHORIZED,
                "Invalid Authorization header format",
            ),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid or expired token"),
        };

        (
            status,
            Json(ErrorResponse {
                error: message.to_string(),
            }),
        )
            .into_response()
    }
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    Arc<DbPool>: FromRef<S>,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = Arc::<DbPool>::from_ref(state);

        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .ok_or(AuthError::MissingHeader)?;

        let auth_str = auth_header.to_str().map_err(|_| AuthError::InvalidHeader)?;

        let token = auth_str
            .strip_prefix("Bearer ")
            .ok_or(AuthError::InvalidFormat)?;

        let user = get_user_from_token(&pool, token)
            .await
            .ok_or(AuthError::InvalidToken)?;

        Ok(AuthUser(user))
    }
}
