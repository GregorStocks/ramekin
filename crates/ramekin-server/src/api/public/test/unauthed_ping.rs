use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

pub const PATH: &str = "/api/test/unauthed-ping";

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Response {
    pub message: String,
}

#[utoipa::path(
    get,
    path = "/api/test/unauthed-ping",
    responses(
        (status = 200, description = "Unauthed ping response", body = Response)
    )
)]
pub async fn handler() -> Json<Response> {
    Json(Response {
        message: "unauthed-ping".to_string(),
    })
}
