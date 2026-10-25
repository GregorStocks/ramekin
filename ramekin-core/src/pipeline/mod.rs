//! Generic pipeline framework for recipe scraping and normalizing.
//!
//! This module provides a trait-based pipeline system where:
//! - Steps are defined via the `PipelineStep` trait
//! - Each step returns `next_step` to indicate what runs next (duck typing)
//! - CLI and server build their own registries with appropriate implementations
//! - DB-specific steps are abstract here (metadata only), implemented in cli/server

mod executor;
mod normalize_ops;
mod step;
pub mod steps;

pub use executor::{run_pipeline, StepRegistry};
pub use normalize_ops::{
    default_import_ops, normalize_phase_steps, op_for_step, order_ops, NormalizeOp,
    NormalizeOpGroup, APPLY_NORMALIZE_STEP, LOAD_RECIPE_STEP, PROPOSE_NORMALIZE_STEP,
    RESOLVE_INGREDIENT_NAMES_STEP,
};
pub use step::{
    deserialize_optional_output_field, deserialize_required_output_field, PipelineStep,
    StepContext, StepMetadata, StepOutputStore, StepResult,
};

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::steps::{
        AutoTagStep, ExtractRecipeStep, FetchHtmlStep, FetchImagesStepMeta, SaveRecipeStepMeta,
    };
    use crate::MockClient;

    #[test]
    fn step_names_are_unique() {
        // We need a mock client for FetchHtmlStep
        let mock_client = MockClient::default();
        let _fetch_html = FetchHtmlStep::new(mock_client);

        let names = [
            FetchHtmlStep::<MockClient>::NAME,
            ExtractRecipeStep::NAME,
            FetchImagesStepMeta::NAME,
            SaveRecipeStepMeta::NAME,
            AutoTagStep::NAME,
        ];

        let unique: HashSet<_> = names.iter().collect();
        assert_eq!(
            names.len(),
            unique.len(),
            "Duplicate step names detected! Names: {:?}",
            names
        );
    }
}
