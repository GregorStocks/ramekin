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
    default_import_ops, normalize_phase_steps, NormalizeOp, APPLY_NORMALIZE_STEP,
    EDIT_WITH_AI_STEP, LOAD_RECIPE_STEP, PROPOSE_NORMALIZE_STEP, RESOLVE_INGREDIENT_NAMES_STEP,
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
    /// The normalize ops the job runs, in run order.
    pub ops: Vec<NormalizeOp>,
    /// Whether `fetch_html` must skip the HTTP cache: the job re-reads an
    /// existing recipe's source, so it must see the page as it is now.
    pub refetch: bool,
}

/// The steps that read a recipe from its source URL.
const SOURCE_STEPS: &[&str] = &[
    FetchHtmlStep::NAME,
    ExtractRecipeStep::NAME,
    FetchImagesStepMeta::NAME,
    ParseIngredientsStep::NAME,
];

fn scrape_steps() -> Vec<&'static str> {
    let mut steps = SOURCE_STEPS.to_vec();
    steps.push(SaveRecipeStepMeta::NAME);
    steps
}

impl JobPlan {
    pub fn for_job(job: &ScrapeJob) -> Result<Self, String> {
        if job.edit_request.is_some() {
            return Ok(Self::edit());
        }
        let rescrape = job.expected_version_id.is_some();
        match &job.normalize_ops {
            Some(ops) => Ok(Self::normalize(&ops_from_json(ops)?)),
            None if job.photo_only => Ok(Self {
                sequential: scrape_steps(),
                // Photo-only rescrape must not run normalize ops: they could
                // write another version after the photo-only update.
                concurrent: Vec::new(),
                ops: Vec::new(),
                refetch: true,
            }),
            None => Ok(Self {
                refetch: rescrape,
                ..Self::import()
            }),
        }
    }

    /// A new recipe from a URL, capture or import.
    pub fn import() -> Self {
        let ops = default_import_ops();
        let mut concurrent = vec![(RESOLVE_INGREDIENT_NAMES_STEP, Vec::new())];
        concurrent.extend(normalize_phase_steps(&ops, APPLY_NORMALIZE_STEP));
        Self {
            sequential: scrape_steps(),
            concurrent,
            ops,
            refetch: false,
        }
    }

    /// A manual normalize run of `ops`, proposing a version.
    pub fn normalize(ops: &[NormalizeOp]) -> Self {
        let sequential = if ops.contains(&NormalizeOp::Refetch) {
            SOURCE_STEPS.to_vec()
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
            ops: ops.to_vec(),
            refetch: true,
        }
    }

    /// An Edit with AI run on the base version, proposing a version.
    pub fn edit() -> Self {
        Self {
            sequential: vec![LOAD_RECIPE_STEP],
            concurrent: vec![
                (EDIT_WITH_AI_STEP, Vec::new()),
                (PROPOSE_NORMALIZE_STEP, vec![EDIT_WITH_AI_STEP]),
            ],
            ops: Vec::new(),
            refetch: false,
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
        self.sequential.contains(&step_name) || self.is_concurrent(step_name)
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

    #[test]
    fn edit_plans_load_edit_then_propose() {
        let edit = JobPlan::edit();
        assert_eq!(
            edit.step_names(),
            vec!["load_recipe", "edit_with_ai", "propose_normalize"]
        );
        assert!(edit.ops.is_empty());
    }
}
