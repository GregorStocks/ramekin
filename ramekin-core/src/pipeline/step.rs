//! Pipeline step trait and supporting types.

use std::error::Error;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Metadata about a pipeline step.
#[derive(Debug, Clone)]
pub struct StepMetadata {
    /// Unique identifier for this step (e.g., "fetch_html", "extract_recipe")
    pub name: &'static str,
    /// Human-readable description
    pub description: &'static str,
    /// If true, failures don't fail the overall pipeline
    pub continues_on_failure: bool,
}

/// Result of executing a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Name of the step that produced this result
    pub step_name: String,
    /// Whether the step succeeded
    pub success: bool,
    /// The output data (JSON)
    pub output: JsonValue,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// How long the step took in milliseconds
    pub duration_ms: u64,
    /// Name of the next step to run (duck typing - step decides what's next)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_step: Option<String>,
}

/// Abstraction for reading/writing step outputs.
/// Implemented differently by CLI (files) vs server (DB).
pub trait StepOutputStore: Send + Sync {
    /// Get the output from a previous step by name.
    fn get_output(&self, step_name: &str) -> Option<JsonValue>;

    /// Save the output from a step.
    fn save_output(
        &mut self,
        step_name: &str,
        output: &JsonValue,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}

/// Context provided to steps during execution.
pub struct StepContext<'a> {
    /// URL being processed
    pub url: &'a str,
    /// Access to prior step outputs
    pub outputs: &'a dyn StepOutputStore,
}

/// The main trait for pipeline steps.
#[async_trait]
pub trait PipelineStep: Send + Sync {
    /// Return metadata about this step.
    fn metadata(&self) -> StepMetadata;

    /// Execute the step.
    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult;
}
