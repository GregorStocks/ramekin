//! Which steps a job runs, and in what order.
//!
//! Every job runs some steps one after another, then some concurrently as
//! their dependencies finish. Imports and scrapes run the source steps,
//! `save_recipe`, then the default normalize ops and `apply_normalize`. A
//! manual normalize job runs its source ops' steps (or `load_recipe`), then
//! its normalize ops and `propose_normalize`.

use ramekin_core::pipeline::steps::{
    ExtractRecipeStep, FetchImagesStepMeta, ParseIngredientsStep, SaveRecipeStepMeta,
};
use ramekin_core::pipeline::{
    default_import_ops, normalize_phase_steps, NormalizeOp, APPLY_NORMALIZE_STEP, LOAD_RECIPE_STEP,
    PROPOSE_NORMALIZE_STEP, RESOLVE_INGREDIENT_NAMES_STEP,
};

use crate::models::ScrapeJob;
use crate::normalize::ops_from_json;

use super::steps::FetchHtmlStep;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobPlan {
    /// Steps that run one after another, in order.
    pub sequential: Vec<&'static str>,
    /// Steps after the sequential ones, in canonical order, each with the
    /// steps it waits for.
    pub concurrent: Vec<(&'static str, Vec<&'static str>)>,
}

const SCRAPE_STEPS: &[&str] = &[
    FetchHtmlStep::NAME,
    ExtractRecipeStep::NAME,
    FetchImagesStepMeta::NAME,
    ParseIngredientsStep::NAME,
    SaveRecipeStepMeta::NAME,
];

impl JobPlan {
    pub fn for_job(job: &ScrapeJob) -> Result<Self, String> {
        match &job.normalize_ops {
            Some(ops) => Ok(Self::normalize(&ops_from_json(ops)?)),
            None if job.photo_only => Ok(Self {
                sequential: SCRAPE_STEPS.to_vec(),
                // Photo-only rescrape must not run normalize ops: they could
                // write another version after the photo-only update.
                concurrent: Vec::new(),
            }),
            None => Ok(Self::import()),
        }
    }

    /// A new recipe from a URL, capture or import, or a rescrape.
    pub fn import() -> Self {
        let mut concurrent = vec![(RESOLVE_INGREDIENT_NAMES_STEP, Vec::new())];
        concurrent.extend(normalize_phase_steps(
            &default_import_ops(),
            APPLY_NORMALIZE_STEP,
        ));
        Self {
            sequential: SCRAPE_STEPS.to_vec(),
            concurrent,
        }
    }

    /// A manual normalize run of `ops`, proposing a version.
    pub fn normalize(ops: &[NormalizeOp]) -> Self {
        let sequential = if ops.contains(&NormalizeOp::Refetch) {
            vec![
                FetchHtmlStep::NAME,
                ExtractRecipeStep::NAME,
                FetchImagesStepMeta::NAME,
                ParseIngredientsStep::NAME,
            ]
        } else if ops.contains(&NormalizeOp::Reextract) {
            // Starts at parse_ingredients when the source has no page to
            // extract from (Paprika and photo imports).
            vec![ExtractRecipeStep::NAME, ParseIngredientsStep::NAME]
        } else {
            vec![LOAD_RECIPE_STEP]
        };
        Self {
            sequential,
            concurrent: normalize_phase_steps(ops, PROPOSE_NORMALIZE_STEP),
        }
    }

    /// Every step, in canonical order.
    pub fn step_names(&self) -> Vec<&'static str> {
        self.sequential
            .iter()
            .copied()
            .chain(self.concurrent.iter().map(|(name, _)| *name))
            .collect()
    }

    pub fn contains(&self, step_name: &str) -> bool {
        self.step_names().contains(&step_name)
    }

    pub fn is_concurrent(&self, step_name: &str) -> bool {
        self.concurrent.iter().any(|(name, _)| *name == step_name)
    }

    /// The step after sequential step `step_name`.
    pub fn next_after(&self, step_name: &str) -> Option<&'static str> {
        let names = self.step_names();
        let index = names.iter().position(|name| *name == step_name)?;
        names.get(index + 1).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_saves_then_normalizes() {
        assert_eq!(
            JobPlan::import().step_names(),
            vec![
                "fetch_html",
                "extract_recipe",
                "fetch_images",
                "parse_ingredients",
                "save_recipe",
                "resolve_ingredient_names",
                "normalize_title",
                "generate_description",
                "auto_tag",
                "apply_normalize",
            ]
        );
    }

    #[test]
    fn normalize_plans_start_at_their_source() {
        let refetch = JobPlan::normalize(&[NormalizeOp::Refetch, NormalizeOp::Reextract]);
        assert_eq!(
            refetch.step_names(),
            vec![
                "fetch_html",
                "extract_recipe",
                "fetch_images",
                "parse_ingredients",
                "propose_normalize",
            ]
        );
        let reextract = JobPlan::normalize(&[NormalizeOp::Reextract, NormalizeOp::AutoTag]);
        assert_eq!(
            reextract.step_names(),
            vec![
                "extract_recipe",
                "parse_ingredients",
                "auto_tag",
                "propose_normalize"
            ]
        );
        let title = JobPlan::normalize(&[NormalizeOp::NormalizeTitle]);
        assert_eq!(
            title.step_names(),
            vec!["load_recipe", "normalize_title", "propose_normalize"]
        );
        assert_eq!(title.next_after("load_recipe"), Some("normalize_title"));
        assert!(title.is_concurrent("normalize_title"));
        assert!(!title.is_concurrent("load_recipe"));
    }
}
