//! Pipeline executor and step registry.

use std::collections::HashMap;

use crate::pipeline::step::{PipelineStep, StepContext, StepOutputStore, StepResult};
use crate::types::PipelineStep as ScrapeStep;

/// The names of the steps that scrape a recipe from its source URL, in
/// execution order.
pub fn scrape_steps() -> Vec<&'static str> {
    ScrapeStep::ALL.iter().map(ScrapeStep::as_str).collect()
}

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

/// Run `steps` one after another, in order.
///
/// Stops early when a step fails (unless it has `continues_on_failure`) or
/// isn't registered.
pub async fn run_pipeline(
    steps: &[&str],
    url: &str,
    store: &mut dyn StepOutputStore,
    registry: &StepRegistry,
) -> Vec<StepResult> {
    let mut results = Vec::new();

    for step_name in steps {
        let step = match registry.get(step_name) {
            Some(s) => s,
            None => {
                tracing::error!("Pipeline step {} is not registered", step_name);
                break;
            }
        };

        let meta = step.metadata();
        let ctx = StepContext {
            url,
            outputs: store,
        };
        let mut result = step.execute(&ctx).await;

        if let Err(error) = &result.outcome {
            // The per-URL report truncates errors for grouping, so this is the
            // one place the full error is guaranteed to be visible.
            tracing::warn!(url, step = meta.name, error, "Pipeline step failed");
        }

        // Save output for both success and failure - failure output is needed
        // to root-cause failed runs. The store decides how to record failures.
        if let Err(e) = store
            .save_output(
                meta.name,
                &result.output,
                result.duration_ms as i64,
                result.outcome_ref(),
            )
            .await
        {
            tracing::error!("Failed to save output for step {}: {}", meta.name, e);
            // A successful step whose output can't be saved is treated as
            // failed: later steps read their inputs from the output store, so
            // continuing would run them against silently missing data. An
            // already-failed step keeps its original (more useful) error and
            // stops the pipeline anyway.
            if result.is_success() {
                result.outcome = Err(format!("Failed to save step output: {}", e));
            }
        }

        let should_continue = result.is_success() || meta.continues_on_failure;
        results.push(result);

        if !should_continue {
            break;
        }
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
        saves: Vec<(String, JsonValue, Result<(), String>)>,
    }

    #[async_trait]
    impl StepOutputStore for RecordingStore {
        async fn get_output(&self, _step_name: &str) -> Option<JsonValue> {
            None
        }

        async fn save_output(
            &mut self,
            step_name: &str,
            output: &JsonValue,
            _duration_ms: i64,
            outcome: Result<(), &str>,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.saves.push((
                step_name.to_string(),
                output.clone(),
                outcome.map_err(String::from),
            ));
            Ok(())
        }
    }

    /// Store whose save_output always fails.
    struct FailingStore;

    #[async_trait]
    impl StepOutputStore for FailingStore {
        async fn get_output(&self, _step_name: &str) -> Option<JsonValue> {
            None
        }

        async fn save_output(
            &mut self,
            _step_name: &str,
            _output: &JsonValue,
            _duration_ms: i64,
            _outcome: Result<(), &str>,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Err("disk full".into())
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
                outcome: if self.success {
                    Ok(())
                } else {
                    Err("AI call failed: something specific".to_string())
                },
                output: if self.success {
                    json!({ "value": 42 })
                } else {
                    json!({ "error": "AI call failed: something specific" })
                },
                duration_ms: 1,
            }
        }
    }

    /// Step that always succeeds.
    struct AlwaysOkStep;

    #[async_trait]
    impl PipelineStep for AlwaysOkStep {
        fn metadata(&self) -> StepMetadata {
            StepMetadata {
                name: "always_ok",
                description: "succeeds",
                continues_on_failure: false,
            }
        }

        async fn execute(&self, _ctx: &StepContext<'_>) -> StepResult {
            StepResult {
                step_name: "always_ok".to_string(),
                output: json!({"ok": true}),
                outcome: Ok(()),
                duration_ms: 0,
            }
        }
    }

    /// Step that panics if executed — used to assert the pipeline stopped.
    struct UnreachableStep;

    #[async_trait]
    impl PipelineStep for UnreachableStep {
        fn metadata(&self) -> StepMetadata {
            StepMetadata {
                name: "never_reached",
                description: "must not run",
                continues_on_failure: false,
            }
        }

        async fn execute(&self, _ctx: &StepContext<'_>) -> StepResult {
            panic!("pipeline continued past a step whose output failed to save");
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

        run_pipeline(&["ok_step"], "https://example.com", &mut store, &registry).await;

        assert_eq!(store.saves.len(), 1);
        let (name, output, outcome) = &store.saves[0];
        assert_eq!(name, "ok_step");
        assert_eq!(output, &json!({ "value": 42 }));
        assert_eq!(outcome, &Ok(()));
    }

    #[tokio::test]
    async fn failed_step_output_is_saved_with_error() {
        let mut registry = StepRegistry::new();
        registry.register(Box::new(FixedStep {
            name: "bad_step",
            success: false,
        }));
        let mut store = RecordingStore::default();

        run_pipeline(&["bad_step"], "https://example.com", &mut store, &registry).await;

        assert_eq!(store.saves.len(), 1);
        let (name, output, outcome) = &store.saves[0];
        assert_eq!(name, "bad_step");
        assert_eq!(
            output,
            &json!({ "error": "AI call failed: something specific" })
        );
        assert_eq!(
            outcome,
            &Err("AI call failed: something specific".to_string())
        );
    }

    #[tokio::test]
    async fn save_failure_fails_the_step_and_stops_the_pipeline() {
        let mut registry = StepRegistry::new();
        registry.register(Box::new(AlwaysOkStep));
        registry.register(Box::new(UnreachableStep));
        let mut store = FailingStore;

        let results = run_pipeline(
            &["always_ok", "never_reached"],
            "http://example.com",
            &mut store,
            &registry,
        )
        .await;

        assert_eq!(results.len(), 1);
        let result = &results[0];
        assert_eq!(
            result.error(),
            Some("Failed to save step output: disk full")
        );
    }
}
