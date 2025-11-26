use crate::api::ErrorResponse;
use crate::db::DbPool;
use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
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
    let auth_header = match request.headers().get(header::AUTHORIZATION) {
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

    // Validate token
    if get_user_from_token(&pool, token).await.is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Invalid or expired token".to_string(),
            }),
        )
            .into_response();
    }

    next.run(request).await
}
