use std::time::Instant;

use serde_json::json;
use uuid::Uuid;

use ramekin_core::pipeline::{
    deserialize_required_output_field, step_after_scrape_auto_applied_ai_step, StepContext,
    StepResult,
};

pub(super) enum SaveOutputReadError {
    MissingSaveRecipeOutput,
    MissingRecipeId,
    MalformedRecipeId(String),
}

pub(super) trait SaveOutputReadErrorExt {
    fn with_step(self, step_name: &str, start: Instant, next_from: &str) -> StepResult;
}

impl SaveOutputReadErrorExt for SaveOutputReadError {
    fn with_step(self, step_name: &str, start: Instant, next_from: &str) -> StepResult {
        let message = match self {
            SaveOutputReadError::MissingSaveRecipeOutput => "save_recipe output not found",
            SaveOutputReadError::MissingRecipeId => "No recipe_id in save_recipe output",
            SaveOutputReadError::MalformedRecipeId(ref e) => e,
        };
        StepResult {
            step_name: step_name.to_string(),
            success: false,
            output: json!({ "error": message }),
            error: Some(message.to_string()),
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: step_after_scrape_auto_applied_ai_step(next_from).map(str::to_string),
        }
    }
}

pub(super) async fn recipe_id_from_save_output(
    ctx: &StepContext<'_>,
) -> Result<Uuid, SaveOutputReadError> {
    let output = ctx
        .outputs
        .get_output("save_recipe")
        .await
        .ok_or(SaveOutputReadError::MissingSaveRecipeOutput)?;
    let raw_id: String = deserialize_required_output_field(&output, "save_recipe", "recipe_id")
        .map_err(|e| {
            if e.starts_with("Missing ") {
                SaveOutputReadError::MissingRecipeId
            } else {
                SaveOutputReadError::MalformedRecipeId(e)
            }
        })?;

    Uuid::parse_str(&raw_id).map_err(|e| {
        SaveOutputReadError::MalformedRecipeId(format!(
            "Malformed recipe_id in save_recipe output: {e}"
        ))
    })
}

/// Find the newest version produced by an earlier pipeline write. Candidates
/// must be ordered newest-first and restricted to steps that precede the
/// caller so stale outputs from a later retry step cannot be selected.
pub(super) async fn version_id_from_pipeline_outputs(
    ctx: &StepContext<'_>,
    candidates: &[(&str, &str)],
) -> Result<Uuid, String> {
    for (step_name, field_name) in candidates {
        let Some(output) = ctx.outputs.get_output(step_name).await else {
            continue;
        };
        let Some(value) = output.get(field_name) else {
            continue;
        };
        let raw_id = value
            .as_str()
            .ok_or_else(|| format!("{field_name} in {step_name} output must be a UUID string"))?;
        return Uuid::parse_str(raw_id)
            .map_err(|error| format!("Malformed {field_name} in {step_name} output: {error}"));
    }

    Err("No source recipe version found in prior pipeline outputs".to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use async_trait::async_trait;
    use ramekin_core::pipeline::StepOutputStore;
    use serde_json::{json, Value};

    use super::*;

    struct Outputs(HashMap<String, Value>);

    #[async_trait]
    impl StepOutputStore for Outputs {
        async fn get_output(&self, step_name: &str) -> Option<Value> {
            self.0.get(step_name).cloned()
        }

        async fn save_output(
            &mut self,
            _step_name: &str,
            _output: &Value,
            _duration_ms: i64,
            _success: bool,
            _error: Option<&str>,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            unreachable!("read-only test output store")
        }
    }

    #[tokio::test]
    async fn pipeline_version_uses_newest_allowed_predecessor() {
        let saved = Uuid::new_v4();
        let normalized = Uuid::new_v4();
        let stale_later_output = Uuid::new_v4();
        let outputs = Outputs(HashMap::from([
            (
                "save_recipe".to_string(),
                json!({ "version_id": saved.to_string() }),
            ),
            (
                "apply_normalized_title".to_string(),
                json!({ "new_version_id": normalized.to_string() }),
            ),
            (
                "apply_auto_tags".to_string(),
                json!({ "new_version_id": stale_later_output.to_string() }),
            ),
        ]));
        let context = StepContext {
            url: "",
            outputs: &outputs,
        };

        let selected = version_id_from_pipeline_outputs(
            &context,
            &[
                ("apply_normalized_title", "new_version_id"),
                ("save_recipe", "version_id"),
            ],
        )
        .await
        .expect("a predecessor version should be selected");

        assert_eq!(selected, normalized);
        assert_ne!(selected, stale_later_output);
    }

    #[tokio::test]
    async fn pipeline_version_falls_back_when_predecessor_did_not_write() {
        let saved = Uuid::new_v4();
        let outputs = Outputs(HashMap::from([
            (
                "save_recipe".to_string(),
                json!({ "version_id": saved.to_string() }),
            ),
            (
                "apply_normalized_title".to_string(),
                json!({ "changed": false }),
            ),
        ]));
        let context = StepContext {
            url: "",
            outputs: &outputs,
        };

        let selected = version_id_from_pipeline_outputs(
            &context,
            &[
                ("apply_normalized_title", "new_version_id"),
                ("save_recipe", "version_id"),
            ],
        )
        .await
        .expect("the save version should be used");

        assert_eq!(selected, saved);
    }
}
