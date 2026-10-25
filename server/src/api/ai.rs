use super::ApiError;
use ramekin_core::ai::AiConfig;

pub(crate) fn ai_config_from_env() -> Result<AiConfig, ApiError> {
    AiConfig::from_env().map_err(|e| {
        tracing::warn!("AI config unavailable: {}", e);
        ApiError::service_unavailable("AI service unavailable")
    })
}
