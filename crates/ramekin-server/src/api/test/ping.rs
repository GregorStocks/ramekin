use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use axum::{response::IntoResponse, Json};
use serde::Serialize;
use utoipa::ToSchema;

pub const PATH: &str = "/api/test/ping";

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Response {
    pub message: String,
}

#[utoipa::path(
    get,
    path = "/api/test/ping",
    responses(
        (status = 200, description = "Authenticated ping response", body = Response),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn ping(AuthUser(_user): AuthUser) -> impl IntoResponse {
    Json(Response {
        message: "ping".to_string(),
    })
}
