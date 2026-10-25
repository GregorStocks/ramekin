use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use ramekin_core::pipeline::{
    NormalizeOp, NormalizeOpGroup, PipelineStep, StepContext, StepMetadata, StepResult,
    PROPOSE_NORMALIZE_STEP,
};

use crate::db::DbPool;
use crate::ids::Uuid;
use crate::normalize::{apply_op_outputs, load_version_blocking, source_recipe, NormalizedRecipe};

use super::helpers::step_failure;

/// The output of `propose_normalize`: what a manual normalize run would save
/// over the job's base version (`scrape_jobs.expected_version_id`).
#[derive(Debug, Serialize, Deserialize)]
pub struct NormalizeProposal {
    pub proposed: NormalizedRecipe,
}

/// Builds the recipe a manual normalize job proposes: the source ops'
/// rebuilt recipe (or the base version), with each normalize op's output
/// applied. Saves nothing; the user accepts or discards the proposal.
pub struct ProposeNormalizeStep {
    pool: Arc<DbPool>,
    base_version_id: Uuid,
    ops: Vec<NormalizeOp>,
}

impl ProposeNormalizeStep {
    pub fn new(pool: Arc<DbPool>, base_version_id: Uuid, ops: Vec<NormalizeOp>) -> Self {
        Self {
            pool,
            base_version_id,
            ops,
        }
    }
}

#[async_trait]
impl PipelineStep for ProposeNormalizeStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: PROPOSE_NORMALIZE_STEP,
            description: "Build the proposed recipe version",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();
        let base = match load_version_blocking(&self.pool, self.base_version_id).await {
            Ok(recipe) => recipe,
            Err(error) => return step_failure(PROPOSE_NORMALIZE_STEP, start, error),
        };

        let mut proposed = if self
            .ops
            .iter()
            .any(|op| op.group() == NormalizeOpGroup::Source)
        {
            match source_recipe(ctx.outputs, &base).await {
                Ok(recipe) => recipe,
                Err(error) => return step_failure(PROPOSE_NORMALIZE_STEP, start, error),
            }
        } else {
            base
        };
        if let Err(error) = apply_op_outputs(&mut proposed.content, &self.ops, ctx.outputs).await {
            return step_failure(PROPOSE_NORMALIZE_STEP, start, error);
        }

        let proposal = NormalizeProposal { proposed };
        StepResult {
            step_name: PROPOSE_NORMALIZE_STEP.to_string(),
            success: true,
            output: serde_json::to_value(proposal).expect("NormalizeProposal serializes to JSON"),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: None,
        }
    }
}
