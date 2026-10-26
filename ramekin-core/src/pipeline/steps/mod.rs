//! Pipeline step implementations.
//!
//! Generic steps are fully implemented here. DB-specific steps only have metadata.

mod auto_tag;
mod extract_recipe;
mod fetch_html;
mod fetch_images;
mod generate_description;
mod house_style;
mod normalize_title;
mod parse_ingredients;
mod save_recipe;

pub use auto_tag::AutoTagStep;
pub use extract_recipe::ExtractRecipeStep;
pub use fetch_html::FetchHtmlStep;
pub use fetch_images::FetchImagesStepMeta;
pub use generate_description::GenerateDescriptionStep;
pub use house_style::HouseStyleStep;
pub use normalize_title::NormalizeTitleStep;
pub use parse_ingredients::{parse_raw_ingredients, ParseIngredientsStep};
pub use save_recipe::SaveRecipeStepMeta;

use std::time::Instant;

use serde::de::DeserializeOwned;
use serde_json::json;

use crate::pipeline::{
    deserialize_required_output_field, StepContext, StepResult, LOAD_RECIPE_STEP,
};
use crate::types::RawRecipe;

/// The recipe a normalize step works on: the job's freshly extracted recipe,
/// or the current version when the job loaded it instead of re-reading the
/// source.
pub(crate) async fn base_raw_recipe(ctx: &StepContext<'_>) -> Result<RawRecipe, String> {
    base_output_field(ctx, ExtractRecipeStep::NAME, "raw_recipe").await
}

/// `field` of the recipe a normalize step works on: from `fresh_step` when
/// the job read the source, or from `load_recipe` when it loaded the current
/// version instead. A job has one or the other: imports and source ops
/// extract, and a job without a source op loads the current version.
pub(crate) async fn base_output_field<T: DeserializeOwned>(
    ctx: &StepContext<'_>,
    fresh_step: &str,
    field: &str,
) -> Result<T, String> {
    for step_name in [fresh_step, LOAD_RECIPE_STEP] {
        if let Some(output) = ctx.outputs.get_output(step_name).await {
            return deserialize_required_output_field(&output, step_name, field);
        }
    }
    Err(format!(
        "No {LOAD_RECIPE_STEP} or {fresh_step} output found"
    ))
}

/// A step output store backed by a map, for step tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct TestOutputStore {
    pub outputs: std::collections::HashMap<String, serde_json::Value>,
}

#[cfg(test)]
#[async_trait::async_trait]
impl crate::pipeline::StepOutputStore for TestOutputStore {
    async fn get_output(&self, step_name: &str) -> Option<serde_json::Value> {
        self.outputs.get(step_name).cloned()
    }

    async fn save_output(
        &mut self,
        _step_name: &str,
        _output: &serde_json::Value,
        _duration_ms: i64,
        _success: bool,
        _error: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

/// A failed step's result.
pub fn step_failure(step_name: &str, start: Instant, error: String) -> StepResult {
    StepResult {
        step_name: step_name.to_string(),
        success: false,
        output: json!({ "error": error }),
        error: Some(error),
        duration_ms: start.elapsed().as_millis() as u64,
        next_step: None,
    }
}
