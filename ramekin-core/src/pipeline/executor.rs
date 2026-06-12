//! Pipeline executor and step registry.

use std::collections::HashMap;

use crate::pipeline::step::{PipelineStep, StepContext, StepOutputStore, StepResult};

/// Registry that maps step names to their implementations.
pub struct StepRegistry {
    steps: HashMap<String, Box<dyn PipelineStep>>,
}

impl StepRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            steps: HashMap::new(),
        }
    }

    /// Register a step implementation.
    pub fn register(&mut self, step: Box<dyn PipelineStep>) {
        self.steps.insert(step.metadata().name.to_string(), step);
    }

    /// Get a step by name.
    pub fn get(&self, name: &str) -> Option<&dyn PipelineStep> {
        self.steps.get(name).map(|s| s.as_ref())
    }
}

impl Default for StepRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Run a pipeline starting from the given step.
///
/// The executor follows the step-driven chain: each step returns `next_step`
/// to indicate what should run next. This continues until a step returns
/// `next_step: None` or a step fails (unless it has `continues_on_failure`).
pub async fn run_pipeline(
    first_step_name: &str,
    url: &str,
    store: &mut dyn StepOutputStore,
    registry: &StepRegistry,
) -> Vec<StepResult> {
    let mut results = Vec::new();
    let mut current_step_name = Some(first_step_name.to_string());

    while let Some(step_name) = current_step_name {
        let step = match registry.get(&step_name) {
            Some(s) => s,
            None => break, // Unknown step, stop
        };

        let meta = step.metadata();
        let ctx = StepContext {
            url,
            outputs: store,
        };
        let mut result = step.execute(&ctx).await;

        if !result.success {
            // The per-URL report truncates errors for grouping, so this is the
            // one place the full error is guaranteed to be visible.
            tracing::warn!(
                url,
                step = meta.name,
                error = result.error.as_deref().unwrap_or("unknown error"),
                "Pipeline step failed"
            );
        }

        // Save output for both success and failure - failure output is needed
        // to root-cause failed runs. The store decides how to record failures.
        if let Err(e) = store.save_output(
            meta.name,
            &result.output,
            result.duration_ms as i64,
            result.success,
            result.error.as_deref(),
        ) {
            tracing::error!("Failed to save output for step {}: {}", meta.name, e);
            // A successful step whose output can't be saved is treated as
            // failed: later steps read their inputs from the output store, so
            // continuing would run them against silently missing data. An
            // already-failed step keeps its original (more useful) error and
            // stops the pipeline anyway.
            if result.success {
                result.success = false;
                result.error = Some(format!("Failed to save step output: {}", e));
            }
        }

        let should_continue = result.success || meta.continues_on_failure;
        let next = result.next_step.clone();
        results.push(result);

        if !should_continue {
            break;
        }

        current_step_name = next;
    }

    results
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde_json::{json, Value as JsonValue};

    use super::*;
    use crate::pipeline::step::StepMetadata;

    /// Store that records every save_output call.
    #[derive(Default)]
    struct RecordingStore {
        saves: Vec<(String, JsonValue, bool, Option<String>)>,
    }

    impl StepOutputStore for RecordingStore {
        fn get_output(&self, _step_name: &str) -> Option<JsonValue> {
            None
        }

        fn save_output(
            &mut self,
            step_name: &str,
            output: &JsonValue,
            _duration_ms: i64,
            success: bool,
            error: Option<&str>,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.saves.push((
                step_name.to_string(),
                output.clone(),
                success,
                error.map(String::from),
            ));
            Ok(())
        }
    }

    /// Step with a fixed result.
    struct FixedStep {
        name: &'static str,
        success: bool,
    }

    #[async_trait]
    impl PipelineStep for FixedStep {
        fn metadata(&self) -> StepMetadata {
            StepMetadata {
                name: self.name,
                description: "test step",
                continues_on_failure: false,
            }
        }

        async fn execute(&self, _ctx: &StepContext<'_>) -> StepResult {
            StepResult {
                step_name: self.name.to_string(),
                success: self.success,
                output: if self.success {
                    json!({ "value": 42 })
                } else {
                    json!({ "error": "AI call failed: something specific" })
                },
                error: (!self.success).then(|| "AI call failed: something specific".to_string()),
                duration_ms: 1,
                next_step: None,
            }
        }
    }

    #[tokio::test]
    async fn successful_step_output_is_saved() {
        let mut registry = StepRegistry::new();
        registry.register(Box::new(FixedStep {
            name: "ok_step",
            success: true,
        }));
        let mut store = RecordingStore::default();

        run_pipeline("ok_step", "https://example.com", &mut store, &registry).await;

        assert_eq!(store.saves.len(), 1);
        let (name, output, success, error) = &store.saves[0];
        assert_eq!(name, "ok_step");
        assert_eq!(output, &json!({ "value": 42 }));
        assert!(*success);
        assert_eq!(error.as_deref(), None);
    }

    #[tokio::test]
    async fn failed_step_output_is_saved_with_error() {
        let mut registry = StepRegistry::new();
        registry.register(Box::new(FixedStep {
            name: "bad_step",
            success: false,
        }));
        let mut store = RecordingStore::default();

        run_pipeline("bad_step", "https://example.com", &mut store, &registry).await;

        assert_eq!(store.saves.len(), 1);
        let (name, output, success, error) = &store.saves[0];
        assert_eq!(name, "bad_step");
        assert_eq!(
            output,
            &json!({ "error": "AI call failed: something specific" })
        );
        assert!(!*success);
        assert_eq!(error.as_deref(), Some("AI call failed: something specific"));
    }
}
