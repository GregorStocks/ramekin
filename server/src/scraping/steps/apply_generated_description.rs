use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use diesel::prelude::*;
use serde_json::json;
use uuid::Uuid;

use ramekin_core::pipeline::{
    step_after_scrape_auto_applied_ai_step, PipelineStep, StepContext, StepMetadata, StepResult,
};

use crate::db::DbPool;
use crate::models::NewRecipeVersion;
use crate::recipes::{create_new_version, TagSource};
use crate::schema::{recipe_versions, recipes};

use super::helpers::{recipe_id_from_save_output, SaveOutputReadErrorExt};

/// Server implementation of ApplyGeneratedDescription step.
pub struct ApplyGeneratedDescriptionStep {
    pool: Arc<DbPool>,
}

impl ApplyGeneratedDescriptionStep {
    pub const NAME: &'static str = "apply_generated_description";

    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PipelineStep for ApplyGeneratedDescriptionStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Apply generated recipe description",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        let recipe_id = match recipe_id_from_save_output(ctx) {
            Ok(id) => id,
            Err(result) => return result.with_step(Self::NAME, start, Self::NAME),
        };

        let description_output = ctx.outputs.get_output("enrich_generate_description");
        let changed = description_output
            .as_ref()
            .and_then(|o| o.get("changed"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let generated_description = description_output
            .as_ref()
            .and_then(|o| o.get("generated_description"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if !changed || generated_description.is_empty() {
            return StepResult {
                step_name: Self::NAME.to_string(),
                success: true,
                output: json!({ "changed": false, "message": "No description change to apply" }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: step_after_scrape_auto_applied_ai_step(Self::NAME).map(str::to_string),
            };
        }

        match self.apply_description(recipe_id, &generated_description) {
            Ok(version_id) => StepResult {
                step_name: Self::NAME.to_string(),
                success: true,
                output: json!({
                    "changed": true,
                    "generated_description": generated_description,
                    "new_version_id": version_id.to_string(),
                }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: step_after_scrape_auto_applied_ai_step(Self::NAME).map(str::to_string),
            },
            Err(e) => StepResult {
                step_name: Self::NAME.to_string(),
                success: false,
                output: json!({ "error": e }),
                error: Some(e),
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: step_after_scrape_auto_applied_ai_step(Self::NAME).map(str::to_string),
            },
        }
    }
}

impl ApplyGeneratedDescriptionStep {
    fn apply_description(&self, recipe_id: Uuid, description: &str) -> Result<Uuid, String> {
        use crate::models::{Recipe, RecipeVersion};

        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        conn.transaction(|conn| {
            let recipe: Recipe = recipes::table.find(recipe_id).first(conn)?;
            let current_version_id = recipe
                .current_version_id
                .ok_or_else(|| diesel::result::Error::RollbackTransaction)?;
            let current: RecipeVersion = recipe_versions::table
                .find(current_version_id)
                .first(conn)?;

            if current.description.as_deref() == Some(description) {
                return Ok(current_version_id);
            }

            let new_version = NewRecipeVersion {
                description: Some(description),
                ..NewRecipeVersion::copy_of(&current, "generate_description")
            };

            let new_version_id =
                create_new_version(conn, &new_version, TagSource::CopyFrom(current_version_id))?;

            Ok(new_version_id)
        })
        .map_err(|e: diesel::result::Error| e.to_string())
    }
}
