//! Pipeline step trait and supporting types.

use std::error::Error;

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;

/// Metadata about a pipeline step.
#[derive(Debug, Clone)]
pub struct StepMetadata {
    /// Unique identifier for this step (e.g., "fetch_html", "extract_recipe")
    pub name: &'static str,
    /// Human-readable description
    pub description: &'static str,
    /// If true, failures don't fail the overall pipeline. This is the only
    /// definition of which steps fail soft; the core executor and the server
    /// runner both read it.
    ///
    /// Normalize steps set this to false: if one fails after `save_recipe`,
    /// the job fails at that step with the saved recipe still attached, so
    /// clients can open the recipe and retry resumes at the failed step.
    /// Only the best-effort post-save step that resolves ingredient names
    /// sets it to true; the job then completes and the status API shows that
    /// step as failed.
    pub continues_on_failure: bool,
}

/// Result of executing a step.
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Name of the step that produced this result
    pub step_name: String,
    /// Whether the step succeeded, or the error it failed with.
    pub outcome: Result<(), String>,
    /// The output data (JSON). A failed step has output too: it is saved to
    /// root-cause failed runs.
    pub output: JsonValue,
    /// How long the step took in milliseconds
    pub duration_ms: u64,
    /// Name of the next step to run (duck typing - step decides what's next)
    pub next_step: Option<String>,
}

impl StepResult {
    pub fn is_success(&self) -> bool {
        self.outcome.is_ok()
    }

    /// The error the step failed with, if it failed.
    pub fn error(&self) -> Option<&str> {
        self.outcome.as_ref().err().map(String::as_str)
    }

    /// The outcome with a borrowed error, as [`StepOutputStore::save_output`]
    /// takes it.
    pub fn outcome_ref(&self) -> Result<(), &str> {
        self.outcome.as_ref().map(|_| ()).map_err(String::as_str)
    }
}

/// Abstraction for reading/writing step outputs.
/// Implemented differently by CLI (files) vs server (DB).
///
/// The methods are async so implementations backed by blocking IO (the
/// server's database store) can hop off the runtime worker instead of
/// parking it.
#[async_trait]
pub trait StepOutputStore: Send + Sync {
    /// Get the output from a previous step by name.
    async fn get_output(&self, step_name: &str) -> Option<JsonValue>;

    /// Save the output from a step.
    ///
    /// `outcome` records whether the step succeeded, or the error it failed
    /// with. It is persisted alongside the output so the status API can
    /// surface per-step failures, including for `continues_on_failure` steps
    /// (the overall job completes, but the individual step still failed).
    async fn save_output(
        &mut self,
        step_name: &str,
        output: &JsonValue,
        duration_ms: i64,
        outcome: Result<(), &str>,
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

/// Deserialize a required field from a step output.
///
/// Missing fields and malformed fields are reported separately so downstream
/// steps don't accidentally treat corrupted upstream data as absent.
pub fn deserialize_required_output_field<T>(
    output: &JsonValue,
    step_name: &str,
    field_name: &str,
) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let value = output
        .get(field_name)
        .ok_or_else(|| format!("Missing {field_name} in {step_name} output"))?;
    serde_json::from_value(value.clone())
        .map_err(|e| format!("Malformed {field_name} in {step_name} output: {e}"))
}

/// Deserialize a required field from an optional step output.
///
/// A missing step output returns `Ok(None)` so callers can keep intentional
/// fallback behavior for skipped prior steps. If the prior output exists, the
/// field itself must be present and valid.
pub fn deserialize_optional_output_field<T>(
    output: Option<&JsonValue>,
    step_name: &str,
    field_name: &str,
) -> Result<Option<T>, String>
where
    T: DeserializeOwned,
{
    output
        .map(|value| deserialize_required_output_field(value, step_name, field_name))
        .transpose()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{deserialize_optional_output_field, deserialize_required_output_field, JsonValue};

    #[test]
    fn required_output_field_reports_missing_field() {
        let err =
            deserialize_required_output_field::<String>(&json!({}), "extract_recipe", "raw_recipe")
                .unwrap_err();

        assert_eq!(err, "Missing raw_recipe in extract_recipe output");
    }

    #[test]
    fn required_output_field_reports_malformed_field() {
        let err = deserialize_required_output_field::<String>(
            &json!({ "raw_recipe": { "title": "Soup" } }),
            "extract_recipe",
            "raw_recipe",
        )
        .unwrap_err();

        assert!(
            err.starts_with("Malformed raw_recipe in extract_recipe output:"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn optional_output_field_allows_missing_output() {
        let got =
            deserialize_optional_output_field::<Vec<String>>(None, "auto_tag", "suggested_tags")
                .unwrap();

        assert_eq!(got, None);
    }

    #[test]
    fn optional_output_field_rejects_malformed_present_output() {
        let output: JsonValue = json!({ "suggested_tags": "dessert" });
        let err = deserialize_optional_output_field::<Vec<String>>(
            Some(&output),
            "auto_tag",
            "suggested_tags",
        )
        .unwrap_err();

        assert!(
            err.starts_with("Malformed suggested_tags in auto_tag output:"),
            "unexpected error: {err}"
        );
    }
}
