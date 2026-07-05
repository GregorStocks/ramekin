use std::time::Instant;

use serde_json::json;
use uuid::Uuid;

use ramekin_core::pipeline::{step_after_scrape_auto_applied_ai_step, StepContext, StepResult};

pub(super) enum SaveOutputReadError {
    MissingRecipeId,
}

pub(super) trait SaveOutputReadErrorExt {
    fn with_step(self, step_name: &str, start: Instant, next_from: &str) -> StepResult;
}

impl SaveOutputReadErrorExt for SaveOutputReadError {
    fn with_step(self, step_name: &str, start: Instant, next_from: &str) -> StepResult {
        let message = match self {
            SaveOutputReadError::MissingRecipeId => "No recipe_id in save_recipe output",
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
    ctx.outputs
        .get_output("save_recipe")
        .as_ref()
        .and_then(|o| o.get("recipe_id"))
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or(SaveOutputReadError::MissingRecipeId)
}
