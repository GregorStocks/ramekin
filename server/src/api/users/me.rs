use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use axum::{response::IntoResponse, Json};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MeResponse {
    pub username: String,
}

#[utoipa::path(
    get,
    path = "/api/users/me",
    tag = "users",
    responses(
        (status = 200, description = "The currently authenticated user", body = MeResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn me(AuthUser(user): AuthUser) -> impl IntoResponse {
    Json(MeResponse {
        username: user.username,
    })
}
