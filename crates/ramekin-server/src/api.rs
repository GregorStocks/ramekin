use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub mod paths {
    pub const GARBAGES: &str = "/api/garbages";
    pub const SIGNUP: &str = "/api/auth/signup";
    pub const LOGIN: &str = "/api/auth/login";
    pub const HELLO: &str = "/api/auth/hello";
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GarbagesResponse {
    pub garbages: Vec<String>,
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
pub struct HelloResponse {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}
