use std::time::Instant;

use crate::ids::Uuid;
use serde_json::json;

use ramekin_core::pipeline::{deserialize_required_output_field, StepContext, StepResult};

use crate::recipes::VersionWriteError;

/// Why an apply step wrote nothing because the recipe moved on without it.
pub(super) const EDITED_SINCE_SAVE: &str = "recipe was edited after it was saved";

/// Treat a stale compare-and-swap as "skip": the recipe is visible as soon as
/// it is saved, so the user may edit it while normalize ops run, and their
/// edit wins over the AI's. Returns `None` when the write was skipped.
pub(super) fn skip_if_edited(
    result: Result<Uuid, VersionWriteError>,
) -> Result<Option<Uuid>, String> {
    match result {
        Ok(version_id) => Ok(Some(version_id)),
        Err(VersionWriteError::Stale) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub(super) enum SaveOutputReadError {
    MissingSaveRecipeOutput,
    MissingRecipeId,
    MalformedRecipeId(String),
}

pub(super) trait SaveOutputReadErrorExt {
    fn with_step(self, step_name: &str, start: Instant) -> StepResult;
}

impl SaveOutputReadErrorExt for SaveOutputReadError {
    fn with_step(self, step_name: &str, start: Instant) -> StepResult {
        let message = match self {
            SaveOutputReadError::MissingSaveRecipeOutput => "save_recipe output not found",
            SaveOutputReadError::MissingRecipeId => "No recipe_id in save_recipe output",
            SaveOutputReadError::MalformedRecipeId(ref e) => e,
        };
        step_failure(step_name, start, message.to_string())
    }
}

/// A failed step's result.
pub(super) fn step_failure(step_name: &str, start: Instant, error: String) -> StepResult {
    StepResult {
        step_name: step_name.to_string(),
        success: false,
        output: json!({ "error": error }),
        error: Some(error),
        duration_ms: start.elapsed().as_millis() as u64,
        next_step: None,
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

/// The version `save_recipe` wrote.
pub(super) async fn saved_version_id(ctx: &StepContext<'_>) -> Result<Uuid, String> {
    let output = ctx
        .outputs
        .get_output("save_recipe")
        .await
        .ok_or("save_recipe output not found")?;
    let raw_id: String = deserialize_required_output_field(&output, "save_recipe", "version_id")?;
    Uuid::parse_str(&raw_id)
        .map_err(|error| format!("Malformed version_id in save_recipe output: {error}"))
}
