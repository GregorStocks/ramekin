use crate::api::ApiError;
use crate::db::DbPool;
use axum::{
    body::Body,
    extract::State,
    http::{header, Method, Request},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

use super::db::get_user_from_token;

/// Middleware that requires a valid auth token for all requests.
/// Apply this to routes that should be protected by default.
pub async fn require_auth(
    State(pool): State<Arc<DbPool>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Allow CORS preflight requests through without auth
    if request.method() == Method::OPTIONS {
        return next.run(request).await;
    }

    let auth_header = match request.headers().get(header::AUTHORIZATION) {
        Some(h) => h,
        None => return ApiError::unauthorized("Missing Authorization header").into_response(),
    };

    let auth_str = match auth_header.to_str() {
        Ok(s) => s,
        Err(_) => return ApiError::unauthorized("Invalid Authorization header").into_response(),
    };

    let token = match auth_str.strip_prefix("Bearer ") {
        Some(t) => t,
        None => {
            return ApiError::unauthorized("Invalid Authorization header format").into_response()
        }
    };

    // Validate token
    if get_user_from_token(&pool, token).await.is_none() {
        return ApiError::unauthorized("Invalid or expired token").into_response();
    }

    next.run(request).await
}
