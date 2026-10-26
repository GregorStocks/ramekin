use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde_json::json;

use ramekin_core::ingredient_parser::ParsedIngredient;
use ramekin_core::pipeline::{
    PipelineStep, StepContext, StepMetadata, StepResult, LOAD_RECIPE_STEP,
};

use crate::db::DbPool;
use crate::ids::Uuid;
use crate::normalize::load_version_blocking;

use super::helpers::step_failure;

/// Writes the version a normalize job started from as its `raw_recipe`, and
/// its structured `ingredients` (as `parse_ingredients` writes them), so
/// normalize ops can run on it without re-reading the source.
pub struct LoadRecipeStep {
    pool: Arc<DbPool>,
    version_id: Uuid,
}

impl LoadRecipeStep {
    pub fn new(pool: Arc<DbPool>, version_id: Uuid) -> Self {
        Self { pool, version_id }
    }
}

#[async_trait]
impl PipelineStep for LoadRecipeStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: LOAD_RECIPE_STEP,
            description: "Load the recipe's current version",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, _ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();
        let recipe = match load_version_blocking(&self.pool, self.version_id).await {
            Ok(recipe) => recipe,
            Err(error) => return step_failure(LOAD_RECIPE_STEP, start, error),
        };
        StepResult {
            step_name: LOAD_RECIPE_STEP.to_string(),
            success: true,
            output: json!({
                "raw_recipe": recipe.to_raw_recipe(),
                "ingredients": recipe
                    .content
                    .ingredients
                    .into_iter()
                    .map(ParsedIngredient::from)
                    .collect::<Vec<_>>(),
            }),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: None,
        }
    }
}
