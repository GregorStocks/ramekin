use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde_json::json;

use ramekin_core::pipeline::{
    NormalizeOp, PipelineStep, StepContext, StepMetadata, StepResult, APPLY_NORMALIZE_STEP,
};

use diesel::prelude::*;
use serde_json::Value as JsonValue;

use crate::db::DbPool;
use crate::ids::Uuid;
use crate::normalize::{apply_op_outputs, load_version_blocking, write_version};
use crate::schema::step_outputs;

use super::helpers::{
    recipe_id_from_save_output, saved_version_id, skip_if_edited, step_failure,
    SaveOutputReadErrorExt, EDITED_SINCE_SAVE,
};

/// Writes one `normalize` version with every normalize op's output on top of
/// the version `save_recipe` just wrote (or that an earlier attempt of this
/// step wrote). Skips the write when the user edited the recipe first: their
/// edit wins over the AI's.
pub struct ApplyNormalizeStep {
    pool: Arc<DbPool>,
    job_id: Uuid,
    user_id: Uuid,
    ops: Vec<NormalizeOp>,
}

impl ApplyNormalizeStep {
    pub fn new(pool: Arc<DbPool>, job_id: Uuid, user_id: Uuid, ops: Vec<NormalizeOp>) -> Self {
        Self {
            pool,
            job_id,
            user_id,
            ops,
        }
    }
}

/// The newest version an earlier run of this step in job `job_id` wrote.
/// Jobs from before `apply_normalize` wrote one version per op, under step
/// names the migration renamed to it, so a retry of such a job builds on
/// what those steps already applied.
fn written_version(conn: &mut diesel::SqliteConnection, job_id: Uuid) -> QueryResult<Option<Uuid>> {
    let outputs: Vec<JsonValue> = step_outputs::table
        .filter(step_outputs::scrape_job_id.eq(job_id))
        .filter(step_outputs::step_name.eq(APPLY_NORMALIZE_STEP))
        .filter(step_outputs::success.eq(true))
        .order(step_outputs::created_at.desc())
        .select(step_outputs::output)
        .load(conn)?;
    Ok(outputs
        .iter()
        .find_map(|output| Uuid::parse_str(output.get("new_version_id")?.as_str()?).ok()))
}

#[async_trait]
impl PipelineStep for ApplyNormalizeStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: APPLY_NORMALIZE_STEP,
            description: "Apply normalized title, description and tags to the recipe",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();
        let recipe_id = match recipe_id_from_save_output(ctx).await {
            Ok(id) => id,
            Err(error) => return error.with_step(APPLY_NORMALIZE_STEP, start),
        };
        let saved_version_id = match saved_version_id(ctx).await {
            Ok(id) => id,
            Err(error) => return step_failure(APPLY_NORMALIZE_STEP, start, error),
        };
        let job_id = self.job_id;
        let base_version_id =
            match crate::db::run_blocking(&self.pool, move |conn| written_version(conn, job_id))
                .await
            {
                Ok(Ok(written)) => written.unwrap_or(saved_version_id),
                Ok(Err(error)) => {
                    return step_failure(APPLY_NORMALIZE_STEP, start, error.to_string())
                }
                Err(error) => return step_failure(APPLY_NORMALIZE_STEP, start, error.to_string()),
            };
        let base = match load_version_blocking(&self.pool, base_version_id).await {
            Ok(recipe) => recipe,
            Err(error) => return step_failure(APPLY_NORMALIZE_STEP, start, error),
        };

        let mut normalized = base.clone();
        if let Err(error) = apply_op_outputs(&mut normalized.content, &self.ops, ctx.outputs).await
        {
            return step_failure(APPLY_NORMALIZE_STEP, start, error);
        }
        if normalized.content == base.content {
            return StepResult {
                step_name: APPLY_NORMALIZE_STEP.to_string(),
                success: true,
                output: json!({ "changed": false }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: None,
            };
        }

        let user_id = self.user_id;
        let ops = self.ops.clone();
        let written = crate::db::run_blocking_write(&self.pool, move |conn| {
            write_version(conn, recipe_id, user_id, base_version_id, &normalized, &ops)
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(skip_if_edited);
        let output = match written {
            Ok(Some(version_id)) => {
                crate::ingredient_names::wake();
                json!({
                "changed": true,
                "new_version_id": version_id.to_string(),
                })
            }
            Ok(None) => json!({ "changed": false, "skipped": EDITED_SINCE_SAVE }),
            Err(error) => return step_failure(APPLY_NORMALIZE_STEP, start, error),
        };
        StepResult {
            step_name: APPLY_NORMALIZE_STEP.to_string(),
            success: true,
            output,
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: None,
        }
    }
}
