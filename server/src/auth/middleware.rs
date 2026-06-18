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
use uuid::Uuid;

use super::db::{get_user_from_token, TOKEN_TYPE_BOOKMARKLET};

/// Whether a bookmarklet-scoped token may access `(method, path)`.
///
/// Bookmarklet tokens are long-lived and embedded in the bookmarklet itself
/// (visible in its saved URL and in the injected `script src` on third-party
/// pages), so they are restricted to exactly what the capture flow needs:
/// - `POST /api/scrape/capture` — upload captured HTML
/// - `GET  /api/scrape/{id}`    — poll job status
/// - `GET  /api/users/me`       — the client's fail-fast auth pre-flight
///
/// Everything else (including minting more tokens) is denied, so a leaked
/// bookmarklet token cannot escalate beyond saving recipes.
fn bookmarklet_scope_allows(method: &Method, path: &str) -> bool {
    match *method {
        Method::POST => path == "/api/scrape/capture",
        Method::GET => path == "/api/users/me" || is_scrape_status_path(path),
        _ => false,
    }
}

/// `/api/scrape/{uuid}` — the job-status poll route. The trailing segment must
/// be a bare UUID, so subpaths (`/retry`, `/steps/...`) and the literal
/// `/capture` are excluded.
fn is_scrape_status_path(path: &str) -> bool {
    match path.strip_prefix("/api/scrape/") {
        Some(rest) => Uuid::parse_str(rest).is_ok(),
        None => false,
    }
}

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
    let (_user, token_type) = match get_user_from_token(&pool, token).await {
        Some(found) => found,
        None => return ApiError::unauthorized("Invalid or expired token").into_response(),
    };

    // Bookmarklet tokens are restricted to the capture flow's endpoints.
    if token_type == TOKEN_TYPE_BOOKMARKLET
        && !bookmarklet_scope_allows(request.method(), request.uri().path())
    {
        return ApiError::forbidden(
            "This bookmarklet token is not permitted to access this endpoint",
        )
        .into_response();
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_capture_upload() {
        assert!(bookmarklet_scope_allows(
            &Method::POST,
            "/api/scrape/capture"
        ));
    }

    #[test]
    fn allows_job_status_poll() {
        let path = format!("/api/scrape/{}", Uuid::nil());
        assert!(bookmarklet_scope_allows(&Method::GET, &path));
    }

    #[test]
    fn allows_me_preflight() {
        assert!(bookmarklet_scope_allows(&Method::GET, "/api/users/me"));
    }

    #[test]
    fn rejects_recipes_listing() {
        assert!(!bookmarklet_scope_allows(&Method::GET, "/api/recipes"));
    }

    #[test]
    fn rejects_minting_more_tokens() {
        // A leaked bookmarklet token must not be able to mint further tokens.
        assert!(!bookmarklet_scope_allows(
            &Method::POST,
            "/api/users/bookmarklet-token"
        ));
    }

    #[test]
    fn rejects_wrong_method_on_capture() {
        assert!(!bookmarklet_scope_allows(
            &Method::GET,
            "/api/scrape/capture"
        ));
        assert!(!bookmarklet_scope_allows(
            &Method::DELETE,
            "/api/scrape/capture"
        ));
    }

    #[test]
    fn rejects_non_uuid_scrape_segment() {
        assert!(!bookmarklet_scope_allows(
            &Method::GET,
            "/api/scrape/not-a-uuid"
        ));
    }

    #[test]
    fn rejects_scrape_subpaths() {
        let retry = format!("/api/scrape/{}/retry", Uuid::nil());
        assert!(!bookmarklet_scope_allows(&Method::POST, &retry));
        let steps = format!("/api/scrape/{}/steps/extract_recipe/output", Uuid::nil());
        assert!(!bookmarklet_scope_allows(&Method::GET, &steps));
    }
}
