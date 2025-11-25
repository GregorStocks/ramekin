use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

pub const PATH: &str = "/api/test/unauthed-ping";

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct UnauthedPingResponse {
    pub message: String,
}

#[utoipa::path(
    get,
    path = "/api/test/unauthed-ping",
    tag = "test",
    responses(
        (status = 200, description = "Unauthed ping response", body = UnauthedPingResponse)
    )
)]
pub async fn unauthed_ping() -> Json<UnauthedPingResponse> {
    Json(UnauthedPingResponse {
        message: "unauthed-ping".to_string(),
    })
}
