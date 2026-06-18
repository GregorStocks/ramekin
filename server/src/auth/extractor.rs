use crate::api::ApiError;
use crate::db::DbPool;
use crate::models::User;
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts},
    response::{IntoResponse, Response},
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
        let message = match self {
            AuthError::MissingHeader => "Missing Authorization header",
            AuthError::InvalidHeader => "Invalid Authorization header",
            AuthError::InvalidFormat => "Invalid Authorization header format",
            AuthError::InvalidToken => "Invalid or expired token",
        };

        ApiError::unauthorized(message).into_response()
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

        // Scope enforcement for bookmarklet tokens happens in `require_auth`,
        // which wraps every protected route; the extractor only needs the user.
        let (user, _token_type) = get_user_from_token(&pool, token)
            .await
            .ok_or(AuthError::InvalidToken)?;

        Ok(AuthUser(user))
    }
}
