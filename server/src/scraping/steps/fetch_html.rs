use std::time::Instant;

use async_trait::async_trait;
use serde_json::json;

use ramekin_core::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};

use super::super::is_host_allowed;

/// Server implementation of FetchHtml step.
///
/// Uses ramekin_core::fetch_html directly (no caching).
pub struct FetchHtmlStep;

impl FetchHtmlStep {
    pub const NAME: &'static str = "fetch_html";
}

#[async_trait]
impl PipelineStep for FetchHtmlStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Fetch HTML from URL",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Check host allowlist
        if let Err(e) = is_host_allowed(ctx.url) {
            return StepResult {
                step_name: Self::NAME.to_string(),
                success: false,
                output: serde_json::Value::Null,
                error: Some(e.to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: None,
            };
        }

        match ramekin_core::fetch_html(ctx.url).await {
            Ok(html) => StepResult {
                step_name: Self::NAME.to_string(),
                success: true,
                output: json!({ "html": html }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: Some("extract_recipe".to_string()),
            },
            Err(e) => StepResult {
                step_name: Self::NAME.to_string(),
                success: false,
                output: serde_json::Value::Null,
                error: Some(e.to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: None,
            },
        }
    }
}
