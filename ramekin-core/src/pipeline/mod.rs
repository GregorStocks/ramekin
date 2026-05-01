//! Generic pipeline framework for recipe scraping and enrichment.
//!
//! This module provides a trait-based pipeline system where:
//! - Steps are defined via the `PipelineStep` trait
//! - Each step returns `next_step` to indicate what runs next (duck typing)
//! - CLI and server build their own registries with appropriate implementations
//! - DB-specific steps are abstract here (metadata only), implemented in cli/server

mod auto_enrichments;
mod executor;
mod step;
pub mod steps;

pub use auto_enrichments::{
    first_scrape_auto_applied_ai_step_name, scrape_auto_applied_ai_enrichments,
    scrape_auto_applied_ai_step_names, scrape_pipeline_step_names,
    step_after_scrape_auto_applied_ai_step, ScrapeAutoAppliedAiEnrichment,
};
pub use executor::{run_pipeline, StepRegistry};
pub use step::{PipelineStep, StepContext, StepMetadata, StepOutputStore, StepResult};

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::steps::{
        EnrichAutoTagStep, ExtractRecipeStep, FetchHtmlStep, FetchImagesStepMeta,
        SaveRecipeStepMeta,
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
