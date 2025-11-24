use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub mod paths {
    pub const UNAUTHED_PING: &str = "/api/test/unauthed-ping";
    pub const PING: &str = "/api/test/ping";
    pub const SIGNUP: &str = "/api/auth/signup";
    pub const LOGIN: &str = "/api/auth/login";
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PingResponse {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SignupRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SignupResponse {
    pub user_id: Uuid,
    pub token: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}
