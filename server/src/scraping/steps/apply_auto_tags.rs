use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use diesel::prelude::*;
use serde_json::json;
use uuid::Uuid;

use ramekin_core::pipeline::{
    deserialize_optional_output_field, step_after_scrape_auto_applied_ai_step, PipelineStep,
    StepContext, StepMetadata, StepResult,
};

use crate::db::{run_blocking, DbPool};
use crate::models::NewRecipeVersion;
use crate::recipes::{create_new_version_cas, TagSource, VersionWriteError};
use crate::schema::{recipe_versions, recipes};

use super::helpers::{
    recipe_id_from_save_output, version_id_from_pipeline_outputs, SaveOutputReadErrorExt,
};

/// Server implementation of ApplyAutoTags step.
///
/// Takes the suggested tags from enrich_auto_tag output and creates a new
/// recipe version with those tags applied.
pub struct ApplyAutoTagsStep {
    pool: Arc<DbPool>,
}

impl ApplyAutoTagsStep {
    pub const NAME: &'static str = "apply_auto_tags";

    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PipelineStep for ApplyAutoTagsStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Apply auto-suggested tags to recipe",
            continues_on_failure: true, // Don't fail the pipeline if tags can't be applied
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Get recipe_id from save_recipe output
        let recipe_id = match recipe_id_from_save_output(ctx).await {
            Ok(id) => id,
            Err(result) => return result.with_step(Self::NAME, start, Self::NAME),
        };

        // Get suggested_tags from enrich_auto_tag output
        let auto_tag_output = ctx.outputs.get_output("enrich_auto_tag").await;
        let suggested_tags: Vec<String> = match deserialize_optional_output_field(
            auto_tag_output.as_ref(),
            "enrich_auto_tag",
            "suggested_tags",
        ) {
            Ok(Some(tags)) => tags,
            Ok(None) => Vec::new(),
            Err(e) => {
                return StepResult {
                    step_name: Self::NAME.to_string(),
                    success: false,
                    output: json!({ "error": e }),
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: step_after_scrape_auto_applied_ai_step(Self::NAME)
                        .map(str::to_string),
                };
            }
        };

        // If no tags suggested, nothing to do
        if suggested_tags.is_empty() {
            return StepResult {
                step_name: Self::NAME.to_string(),
                success: true,
                output: json!({ "message": "No tags to apply", "tags_applied": [] }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: step_after_scrape_auto_applied_ai_step(Self::NAME).map(str::to_string),
            };
        }

        let expected_version_id = match version_id_from_pipeline_outputs(
            ctx,
            &[
                ("apply_generated_description", "new_version_id"),
                ("apply_normalized_title", "new_version_id"),
                ("save_recipe", "version_id"),
            ],
        )
        .await
        {
            Ok(version_id) => version_id,
            Err(error) => {
                return StepResult {
                    step_name: Self::NAME.to_string(),
                    success: false,
                    output: json!({ "error": error }),
                    error: Some(error),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: step_after_scrape_auto_applied_ai_step(Self::NAME)
                        .map(str::to_string),
                };
            }
        };

        // Apply the tags to the recipe
        match self
            .apply_tags(recipe_id, expected_version_id, &suggested_tags)
            .await
        {
            Ok(version_id) => StepResult {
                step_name: Self::NAME.to_string(),
                success: true,
                output: json!({
                    "tags_applied": suggested_tags,
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

impl ApplyAutoTagsStep {
    async fn apply_tags(
        &self,
        recipe_id: Uuid,
        expected_version_id: Uuid,
        new_tags: &[String],
    ) -> Result<Uuid, String> {
        use crate::models::{Recipe, RecipeVersion};

        let new_tags = new_tags.to_vec();
        run_blocking(&self.pool, move |conn| {
            // Get the recipe to find user_id and current_version_id
            let recipe: Recipe = recipes::table
                .find(recipe_id)
                .select(Recipe::as_select())
                .first(conn)
                .map_err(|e| e.to_string())?;

            // Fetch current version data
            let current_version: RecipeVersion = recipe_versions::table
                .filter(recipe_versions::id.eq(expected_version_id))
                .filter(recipe_versions::recipe_id.eq(recipe_id))
                .select(RecipeVersion::as_select())
                .first(conn)
                .map_err(|e| e.to_string())?;

            // Create new version with AI-suggested tags
            conn.transaction(|conn| {
                // 1. Create new version (copy all data, change version_source to "enrichment")
                let new_version = NewRecipeVersion::copy_of(&current_version, "enrichment");

                // 2. Carry existing tags forward and add the AI-suggested ones
                let new_version_id = create_new_version_cas(
                    conn,
                    &new_version,
                    Some(expected_version_id),
                    TagSource::CopyAndNames {
                        from_version: expected_version_id,
                        user_id: recipe.user_id,
                        names: &new_tags,
                    },
                )?;

                Ok(new_version_id)
            })
            .map_err(|e: VersionWriteError| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?
    }
}
