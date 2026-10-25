//! Normalize step - normalize recipe titles.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde::Serialize;

use crate::ai::{normalize_title, AiClient, Usage};
use crate::pipeline::steps::{base_raw_recipe, normalize_step_failure};
use crate::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};

/// Step that normalizes recipe titles using the shared AI title normalizer.
pub struct NormalizeTitleStep {
    ai_client: Arc<dyn AiClient>,
}

impl NormalizeTitleStep {
    pub const NAME: &'static str = "normalize_title";

    pub fn new(ai_client: Arc<dyn AiClient>) -> Self {
        Self { ai_client }
    }
}

#[derive(Debug, Serialize)]
struct NormalizeTitleOutput {
    original_title: String,
    normalized_title: String,
    changed: bool,
    cached: bool,
    usage: Usage,
}

#[async_trait]
impl PipelineStep for NormalizeTitleStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Normalize recipe title",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();
        let raw_recipe = match base_raw_recipe(ctx).await {
            Ok(r) => r,
            Err(e) => return normalize_step_failure(Self::NAME, start, e),
        };

        let title = raw_recipe.title.as_str();
        let result = match normalize_title(
            self.ai_client.as_ref(),
            title,
            &raw_recipe.ingredients,
            &raw_recipe.instructions,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                return normalize_step_failure(Self::NAME, start, format!("AI call failed: {e}"))
            }
        };

        let normalized_title = result.normalized_title.trim().to_string();
        let output = NormalizeTitleOutput {
            original_title: title.to_string(),
            changed: !normalized_title.is_empty() && normalized_title != title,
            normalized_title,
            cached: result.cached,
            usage: result.usage,
        };

        StepResult {
            step_name: Self::NAME.to_string(),
            success: true,
            output: serde_json::to_value(output).expect("NormalizeTitleOutput serializes to JSON"),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::error::Error;
    use std::sync::Arc;

    use async_trait::async_trait;
    use serde_json::{json, Value as JsonValue};

    use super::NormalizeTitleStep;
    use crate::ai::{ConfigError, UnconfiguredAiClient};
    use crate::pipeline::{PipelineStep, StepContext, StepOutputStore};

    #[derive(Default)]
    struct TestOutputStore {
        outputs: HashMap<String, JsonValue>,
    }

    #[async_trait]
    impl StepOutputStore for TestOutputStore {
        async fn get_output(&self, step_name: &str) -> Option<JsonValue> {
            self.outputs.get(step_name).cloned()
        }

        async fn save_output(
            &mut self,
            _step_name: &str,
            _output: &JsonValue,
            _duration_ms: i64,
            _success: bool,
            _error: Option<&str>,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            Ok(())
        }
    }

    /// A missing OPENROUTER_API_KEY reaches the step as an UnconfiguredAiClient
    /// (see server build_registry): the step must fail with the config error,
    /// exactly like any other AI failure.
    #[tokio::test]
    async fn unconfigured_ai_client_fails_step() {
        let mut outputs = TestOutputStore::default();
        outputs.outputs.insert(
            "extract_recipe".to_string(),
            json!({ "raw_recipe": {
                "title": "Soup",
                "ingredients": "water",
                "instructions": "boil",
                "image_urls": [],
            }}),
        );
        let ctx = StepContext {
            url: "https://example.test/soup",
            outputs: &outputs,
        };

        let step = NormalizeTitleStep::new(Arc::new(UnconfiguredAiClient::new(
            ConfigError::MissingEnvVar("OPENROUTER_API_KEY".to_string()),
        )));
        let result = step.execute(&ctx).await;

        assert!(!result.success);
        let err = result.error.unwrap();
        assert!(
            err.contains("OPENROUTER_API_KEY"),
            "unexpected error: {err}"
        );
    }
}
