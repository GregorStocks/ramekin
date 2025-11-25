pub mod photos;
pub mod public;
pub mod test;

use serde::Serialize;
use utoipa::ToSchema;

/// Shared error response used by all endpoints
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}
