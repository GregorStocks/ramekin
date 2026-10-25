use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde_json::json;

use ramekin_core::pipeline::{
    PipelineStep, StepContext, StepMetadata, StepResult, LOAD_RECIPE_STEP,
};

use crate::db::DbPool;
use crate::ids::Uuid;
use crate::normalize::load_version;

use super::helpers::step_failure;

/// Writes the version a normalize job started from as its `raw_recipe`, so
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
        let version_id = self.version_id;
        let recipe =
            match crate::db::run_blocking(&self.pool, move |conn| load_version(conn, version_id))
                .await
            {
                Ok(Ok(recipe)) => recipe,
                Ok(Err(error)) => return step_failure(LOAD_RECIPE_STEP, start, error),
                Err(error) => return step_failure(LOAD_RECIPE_STEP, start, error.to_string()),
            };
        StepResult {
            step_name: LOAD_RECIPE_STEP.to_string(),
            success: true,
            output: json!({ "raw_recipe": recipe.to_raw_recipe() }),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: None,
        }
    }
}
