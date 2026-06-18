use crate::api::{ApiError, ErrorResponse};
use crate::auth::{create_bookmarklet_token, AuthUser};
use crate::db::DbPool;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BookmarkletTokenResponse {
    /// A freshly minted, long-lived token scoped to the capture endpoints.
    /// Embed it in the bookmarklet; it does not expire and does not invalidate
    /// previously minted bookmarklet tokens.
    pub token: String,
}

#[utoipa::path(
    post,
    path = "/api/users/bookmarklet-token",
    tag = "users",
    responses(
        (status = 201, description = "A freshly minted bookmarklet token", body = BookmarkletTokenResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "A bookmarklet token may not mint tokens", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn mint_bookmarklet_token(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
) -> impl IntoResponse {
    // `require_auth` already rejects bookmarklet tokens here (the mint route is
    // not in their allowlist), so this only runs for full session tokens.
    let mut conn = match pool.get() {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!("Failed to get db connection: {}", e);
            return ApiError::internal("Failed to mint bookmarklet token").into_response();
        }
    };

    match create_bookmarklet_token(&mut conn, user.id) {
        Ok(token) => {
            tracing::info!("Minted bookmarklet token for user {}", user.id);
            (
                StatusCode::CREATED,
                Json(BookmarkletTokenResponse { token }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to mint bookmarklet token: {}", e);
            ApiError::internal("Failed to mint bookmarklet token").into_response()
        }
    }
}
