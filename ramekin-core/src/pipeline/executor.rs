//! Pipeline executor and step registry.

use std::collections::HashMap;

use tracing::{info_span, Instrument};

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
        let result = step
            .execute(&ctx)
            .instrument(info_span!("pipeline_step", step = %step_name))
            .await;

        // Save output if successful
        // TODO: Confirm that we want to continue on save failure (vs failing the step)
        if result.success {
            let _save_span = info_span!("save_output", step = %step_name).entered();
            if let Err(e) = store.save_output(meta.name, &result.output) {
                // Log error but continue - we still have the result
                tracing::warn!("Failed to save output for step {}: {}", meta.name, e);
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
