use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use diesel::prelude::*;
use serde_json::json;
use uuid::Uuid;

use ramekin_core::pipeline::{
    deserialize_optional_output_field, deserialize_required_output_field,
    step_after_scrape_auto_applied_ai_step, PipelineStep, StepContext, StepMetadata, StepResult,
};

use crate::db::{run_blocking, DbPool};
use crate::models::NewRecipeVersion;
use crate::recipes::{create_new_version_cas, TagSource, VersionWriteError};
use crate::schema::recipe_versions;

use super::helpers::{
    recipe_id_from_save_output, version_id_from_pipeline_outputs, SaveOutputReadErrorExt,
};

/// Server implementation of ApplyNormalizedTitle step.
pub struct ApplyNormalizedTitleStep {
    pool: Arc<DbPool>,
}

impl ApplyNormalizedTitleStep {
    pub const NAME: &'static str = "apply_normalized_title";

    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PipelineStep for ApplyNormalizedTitleStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Apply normalized recipe title",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        let recipe_id = match recipe_id_from_save_output(ctx) {
            Ok(id) => id,
            Err(result) => return result.with_step(Self::NAME, start, Self::NAME),
        };

        let normalize_output = ctx.outputs.get_output("enrich_normalize_title");
        let changed: bool = match deserialize_optional_output_field(
            normalize_output.as_ref(),
            "enrich_normalize_title",
            "changed",
        ) {
            Ok(Some(changed)) => changed,
            Ok(None) => false,
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

        let normalized_title = if changed {
            match normalize_output
                .as_ref()
                .map(|output| {
                    deserialize_required_output_field::<String>(
                        output,
                        "enrich_normalize_title",
                        "normalized_title",
                    )
                })
                .transpose()
            {
                Ok(Some(title)) => title.trim().to_string(),
                Ok(None) => String::new(),
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
            }
        } else {
            String::new()
        };

        if !changed || normalized_title.is_empty() {
            return StepResult {
                step_name: Self::NAME.to_string(),
                success: true,
                output: json!({ "changed": false, "message": "No title change to apply" }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: step_after_scrape_auto_applied_ai_step(Self::NAME).map(str::to_string),
            };
        }

        let expected_version_id =
            match version_id_from_pipeline_outputs(ctx, &[("save_recipe", "version_id")]) {
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

        match self
            .apply_title(recipe_id, expected_version_id, &normalized_title)
            .await
        {
            Ok(version_id) => StepResult {
                step_name: Self::NAME.to_string(),
                success: true,
                output: json!({
                    "changed": true,
                    "normalized_title": normalized_title,
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

impl ApplyNormalizedTitleStep {
    async fn apply_title(
        &self,
        recipe_id: Uuid,
        expected_version_id: Uuid,
        normalized_title: &str,
    ) -> Result<Uuid, String> {
        use crate::models::RecipeVersion;

        let normalized_title = normalized_title.to_string();
        run_blocking(&self.pool, move |conn| {
            conn.transaction(|conn| {
                let current: RecipeVersion = recipe_versions::table
                    .filter(recipe_versions::id.eq(expected_version_id))
                    .filter(recipe_versions::recipe_id.eq(recipe_id))
                    .select(RecipeVersion::as_select())
                    .first(conn)?;

                if current.title == normalized_title {
                    return Ok(expected_version_id);
                }

                let new_version = NewRecipeVersion {
                    title: &normalized_title,
                    ..NewRecipeVersion::copy_of(&current, "normalize_title")
                };

                let new_version_id = create_new_version_cas(
                    conn,
                    &new_version,
                    Some(expected_version_id),
                    TagSource::CopyFrom(expected_version_id),
                )?;

                Ok(new_version_id)
            })
            .map_err(|e: VersionWriteError| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?
    }
}
