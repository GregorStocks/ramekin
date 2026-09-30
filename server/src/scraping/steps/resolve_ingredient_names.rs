use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use diesel::prelude::*;
use serde_json::json;

use ramekin_core::pipeline::{
    first_scrape_auto_applied_ai_step_name, PipelineStep, StepContext, StepMetadata, StepResult,
    RESOLVE_INGREDIENT_NAMES_STEP,
};

use crate::db::{run_blocking, DbPool};
use crate::ingredient_names::{requeue_failed, resolve_pending, unlearned_names};
use crate::models::Ingredient;
use crate::schema::recipe_versions;

use super::helpers::version_id_from_pipeline_outputs;

/// Resolve the saved recipe's ingredient names the catalog doesn't know
/// (catalog step 3). Runs right after save. The recipe is already saved, so
/// like the enrichment steps a failure shows on the step without failing the
/// job (import jobs can't be retried); the failed names stay visible in
/// Settings, which can retry them.
pub struct ResolveIngredientNamesStep {
    pool: Arc<DbPool>,
}

impl ResolveIngredientNamesStep {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    fn result(start: Instant, outcome: Result<usize, String>) -> StepResult {
        let (success, output, error) = match outcome {
            Ok(names) => (true, json!({ "names": names }), None),
            Err(e) => (false, json!({ "error": e }), Some(e)),
        };
        StepResult {
            step_name: RESOLVE_INGREDIENT_NAMES_STEP.to_string(),
            success,
            output,
            error,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: first_scrape_auto_applied_ai_step_name().map(str::to_string),
        }
    }
}

#[async_trait]
impl PipelineStep for ResolveIngredientNamesStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: RESOLVE_INGREDIENT_NAMES_STEP,
            description: "Resolve ingredient names the catalog doesn't know",
            continues_on_failure: true,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();
        let version_id =
            match version_id_from_pipeline_outputs(ctx, &[("save_recipe", "version_id")]).await {
                Ok(id) => id,
                Err(e) => return Self::result(start, Err(e)),
            };
        let queued = run_blocking(&self.pool, move |conn| {
            let ingredients: serde_json::Value = recipe_versions::table
                .find(version_id)
                .select(recipe_versions::ingredients)
                .first(conn)
                .map_err(|e| e.to_string())?;
            let ingredients: Vec<Ingredient> =
                serde_json::from_value(ingredients).map_err(|e| e.to_string())?;
            let names = unlearned_names(ingredients.iter().map(|i| i.item.as_str()));
            // save_recipe queued these with the recipe. Names that failed
            // before (for an earlier save) are asked about again.
            requeue_failed(conn, &names).map_err(|e| e.to_string())?;
            Ok::<_, String>(names)
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|result| result);
        let names = match queued {
            Ok(names) => names,
            Err(e) => return Self::result(start, Err(e)),
        };
        let count = names.len();
        if count == 0 {
            return Self::result(start, Ok(0));
        }
        Self::result(
            start,
            resolve_pending(&self.pool, Some(names))
                .await
                .map(|()| count),
        )
    }
}
