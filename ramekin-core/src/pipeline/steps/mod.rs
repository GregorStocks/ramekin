//! Pipeline step implementations.
//!
//! Generic steps are fully implemented here. DB-specific steps only have metadata.

mod auto_tag;
mod extract_recipe;
mod fetch_html;
mod fetch_images;
mod generate_description;
mod normalize_title;
mod parse_ingredients;
mod save_recipe;

pub use auto_tag::AutoTagStep;
pub use extract_recipe::ExtractRecipeStep;
pub use fetch_html::FetchHtmlStep;
pub use fetch_images::FetchImagesStepMeta;
pub use generate_description::GenerateDescriptionStep;
pub use normalize_title::NormalizeTitleStep;
pub use parse_ingredients::{parse_raw_ingredients, ParseIngredientsStep};
pub use save_recipe::SaveRecipeStepMeta;

use std::time::Instant;

use serde_json::json;

use crate::pipeline::{
    deserialize_required_output_field, StepContext, StepResult, LOAD_RECIPE_STEP,
};
use crate::types::RawRecipe;

/// The recipe a normalize step works on: the job's freshly extracted recipe,
/// or the current version when the job loaded it instead of re-reading the
/// source.
pub(crate) async fn base_raw_recipe(ctx: &StepContext<'_>) -> Result<RawRecipe, String> {
    // A job has one or the other: imports and source ops extract, and a job
    // without a source op loads the current version.
    for step_name in [ExtractRecipeStep::NAME, LOAD_RECIPE_STEP] {
        if let Some(output) = ctx.outputs.get_output(step_name).await {
            return deserialize_required_output_field(&output, step_name, "raw_recipe");
        }
    }
    Err(format!(
        "No {LOAD_RECIPE_STEP} or {} output found",
        ExtractRecipeStep::NAME
    ))
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
