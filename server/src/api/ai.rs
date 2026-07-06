use super::ApiError;
use ramekin_core::ai::{AiConfig, CachingAiClient};

pub(crate) fn ai_client_from_env() -> Result<CachingAiClient, ApiError> {
    CachingAiClient::from_env().map_err(|e| {
        tracing::warn!("AI client unavailable: {}", e);
        ApiError::service_unavailable("AI service unavailable")
    })
}

pub(crate) fn ai_config_from_env() -> Result<AiConfig, ApiError> {
    AiConfig::from_env().map_err(|e| {
        tracing::warn!("AI config unavailable: {}", e);
        ApiError::service_unavailable("AI service unavailable")
    })
}
