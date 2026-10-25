use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use ramekin_core::pipeline::{
    NormalizeOp, NormalizeOpGroup, PipelineStep, StepContext, StepMetadata, StepResult,
    EDIT_WITH_AI_STEP, PROPOSE_NORMALIZE_STEP,
};

use crate::db::DbPool;
use crate::ids::Uuid;
use crate::normalize::{
    added_tags, apply_op_outputs, load_version_blocking, source_recipe, NormalizedRecipe,
};

use super::edit_with_ai::EditWithAiOutput;
use super::helpers::step_failure;

/// The output of `propose_normalize`: what a manual normalize run would save
/// over the job's base version (`scrape_jobs.expected_version_id`).
#[derive(Debug, Serialize, Deserialize)]
pub struct NormalizeProposal {
    pub proposed: NormalizedRecipe,
    /// The tags `proposed` adds to the base version's.
    pub added_tags: Vec<String>,
    /// What an Edit with AI run says it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// Builds the recipe a manual normalize job proposes: the source ops'
/// rebuilt recipe (or the base version), with each normalize op's output
/// applied, or an Edit with AI job's edited recipe. Saves nothing; the user
/// accepts or discards the proposal.
pub struct ProposeNormalizeStep {
    pool: Arc<DbPool>,
    base_version_id: Uuid,
    ops: Vec<NormalizeOp>,
    /// Propose `edit_with_ai`'s recipe.
    edit: bool,
}

impl ProposeNormalizeStep {
    pub fn new(
        pool: Arc<DbPool>,
        base_version_id: Uuid,
        ops: Vec<NormalizeOp>,
        edit: bool,
    ) -> Self {
        Self {
            pool,
            base_version_id,
            ops,
            edit,
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

        let base_tags = base.content.tags.clone();
        if self.edit {
            let edited: EditWithAiOutput = match ctx.outputs.get_output(EDIT_WITH_AI_STEP).await {
                Some(output) => match serde_json::from_value(output) {
                    Ok(edited) => edited,
                    Err(e) => {
                        return step_failure(
                            PROPOSE_NORMALIZE_STEP,
                            start,
                            format!("Malformed edit_with_ai output: {e}"),
                        )
                    }
                },
                None => {
                    return step_failure(
                        PROPOSE_NORMALIZE_STEP,
                        start,
                        "edit_with_ai output not found".to_string(),
                    )
                }
            };
            let proposed = NormalizedRecipe {
                content: edited.recipe,
                photo_ids: base.photo_ids,
            };
            let proposal = NormalizeProposal {
                added_tags: added_tags(&base_tags, &proposed.content.tags),
                proposed,
                summary: Some(edited.summary),
            };
            return StepResult {
                step_name: PROPOSE_NORMALIZE_STEP.to_string(),
                success: true,
                output: serde_json::to_value(proposal)
                    .expect("NormalizeProposal serializes to JSON"),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: None,
            };
        }
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

        let added_tags = added_tags(&base_tags, &proposed.content.tags);
        let proposal = NormalizeProposal {
            proposed,
            added_tags,
            summary: None,
        };
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
