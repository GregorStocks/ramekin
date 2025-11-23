use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub mod paths {
    pub const GARBAGES: &str = "/api/garbages";
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GarbagesResponse {
    pub garbages: Vec<String>,
}
