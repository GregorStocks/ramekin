use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use axum::{response::IntoResponse, Json};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PingResponse {
    pub message: String,
}

#[utoipa::path(
    get,
    path = "/api/test/ping",
    tag = "testing",
    responses(
        (status = 200, description = "Authenticated ping response", body = PingResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn ping(AuthUser(_user): AuthUser) -> impl IntoResponse {
    Json(PingResponse {
        message: "ping".to_string(),
    })
}
