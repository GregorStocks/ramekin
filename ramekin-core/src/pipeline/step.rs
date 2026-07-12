//! Pipeline step trait and supporting types.

use std::error::Error;

use async_trait::async_trait;
use serde::de::DeserializeOwned;
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
    /// `success` records whether the step succeeded; `error` carries the
    /// step's error message when `success == false`. These are persisted
    /// alongside the output so the status API can surface per-step failures
    /// for enrichment steps that have `continues_on_failure = true` (the
    /// overall job completes, but the individual step still failed).
    async fn save_output(
        &mut self,
        step_name: &str,
        output: &JsonValue,
        duration_ms: i64,
        success: bool,
        error: Option<&str>,
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
        let got = deserialize_optional_output_field::<Vec<String>>(
            None,
            "enrich_auto_tag",
            "suggested_tags",
        )
        .unwrap();

        assert_eq!(got, None);
    }

    #[test]
    fn optional_output_field_rejects_malformed_present_output() {
        let output: JsonValue = json!({ "suggested_tags": "dessert" });
        let err = deserialize_optional_output_field::<Vec<String>>(
            Some(&output),
            "enrich_auto_tag",
            "suggested_tags",
        )
        .unwrap_err();

        assert!(
            err.starts_with("Malformed suggested_tags in enrich_auto_tag output:"),
            "unexpected error: {err}"
        );
    }
}
