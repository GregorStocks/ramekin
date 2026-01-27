//! Generic pipeline framework for recipe scraping and enrichment.
//!
//! This module provides a trait-based pipeline system where:
//! - Steps are defined via the `PipelineStep` trait
//! - Each step returns `next_step` to indicate what runs next (duck typing)
//! - CLI and server build their own registries with appropriate implementations
//! - DB-specific steps are abstract here (metadata only), implemented in cli/server

mod executor;
mod step;
pub mod steps;

pub use executor::{run_pipeline, StepRegistry};
pub use step::{PipelineStep, StepContext, StepMetadata, StepOutputStore, StepResult};

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::steps::{
        EnrichAutoTagStep, EnrichGeneratePhotoStep, EnrichNormalizeIngredientsStep,
        ExtractRecipeStep, FetchHtmlStep, FetchImagesStepMeta, SaveRecipeStepMeta,
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
            EnrichAutoTagStep::NAME,
            EnrichGeneratePhotoStep::NAME,
            EnrichNormalizeIngredientsStep::NAME,
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
