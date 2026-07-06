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

pub(super) fn recipe_id_from_save_output(
    ctx: &StepContext<'_>,
) -> Result<Uuid, SaveOutputReadError> {
    let output = ctx
        .outputs
        .get_output("save_recipe")
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
