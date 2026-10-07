//! Single source of truth for AI enrichments automatically applied by scraping.

/// AI enrichments that scrape jobs should run and apply without user review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrapeAutoAppliedAiEnrichment {
    NormalizeTitle,
    GenerateDescription,
    AutoTag,
}

/// Resolves the saved recipe's ingredient names the catalog doesn't know.
pub const RESOLVE_INGREDIENT_NAMES_STEP: &str = "resolve_ingredient_names";

const CORE_SCRAPE_STEP_NAMES: &[&str] = &[
    "fetch_html",
    "extract_recipe",
    "fetch_images",
    "parse_ingredients",
    "save_recipe",
    RESOLVE_INGREDIENT_NAMES_STEP,
];

const AUTO_TAG_STEP_NAMES: &[&str] = &["enrich_auto_tag", "apply_auto_tags"];
const NORMALIZE_TITLE_STEP_NAMES: &[&str] = &["enrich_normalize_title", "apply_normalized_title"];
const GENERATE_DESCRIPTION_STEP_NAMES: &[&str] =
    &["enrich_generate_description", "apply_generated_description"];

/// Add to this list when a new AI enrichment should be auto-applied at scrape time.
pub const SCRAPE_AUTO_APPLIED_AI_ENRICHMENTS: &[ScrapeAutoAppliedAiEnrichment] = &[
    ScrapeAutoAppliedAiEnrichment::NormalizeTitle,
    ScrapeAutoAppliedAiEnrichment::GenerateDescription,
    ScrapeAutoAppliedAiEnrichment::AutoTag,
];

impl ScrapeAutoAppliedAiEnrichment {
    pub fn step_names(self) -> &'static [&'static str] {
        match self {
            ScrapeAutoAppliedAiEnrichment::NormalizeTitle => NORMALIZE_TITLE_STEP_NAMES,
            ScrapeAutoAppliedAiEnrichment::GenerateDescription => GENERATE_DESCRIPTION_STEP_NAMES,
            ScrapeAutoAppliedAiEnrichment::AutoTag => AUTO_TAG_STEP_NAMES,
        }
    }

    fn enrich_step_name(self) -> &'static str {
        self.step_names()[0]
    }

    fn apply_step_name(self) -> &'static str {
        self.step_names()[1]
    }

    /// Enrichments whose AI output this one's AI call reads.
    fn reads_enrichments(self) -> &'static [ScrapeAutoAppliedAiEnrichment] {
        match self {
            // The description prompt uses the normalized title.
            ScrapeAutoAppliedAiEnrichment::GenerateDescription => {
                &[ScrapeAutoAppliedAiEnrichment::NormalizeTitle]
            }
            ScrapeAutoAppliedAiEnrichment::NormalizeTitle
            | ScrapeAutoAppliedAiEnrichment::AutoTag => &[],
        }
    }
}

pub fn scrape_auto_applied_ai_enrichments() -> &'static [ScrapeAutoAppliedAiEnrichment] {
    SCRAPE_AUTO_APPLIED_AI_ENRICHMENTS
}

pub fn scrape_auto_applied_ai_step_names() -> Vec<&'static str> {
    SCRAPE_AUTO_APPLIED_AI_ENRICHMENTS
        .iter()
        .flat_map(|enrichment| enrichment.step_names().iter().copied())
        .collect()
}

pub fn scrape_pipeline_step_names() -> Vec<&'static str> {
    CORE_SCRAPE_STEP_NAMES
        .iter()
        .copied()
        .chain(scrape_auto_applied_ai_step_names())
        .collect()
}

/// Steps that run after `save_recipe`, in canonical order, each with the
/// steps it must wait for. Scrape jobs run these concurrently as their
/// dependencies finish instead of following `next_step`: the recipe is already
/// saved, and most of them are independent AI calls. Every apply step waits
/// for the previous one because each writes a new recipe version on top of
/// the last.
pub fn scrape_post_save_step_dependencies() -> Vec<(&'static str, Vec<&'static str>)> {
    let enabled = SCRAPE_AUTO_APPLIED_AI_ENRICHMENTS;
    let mut steps = vec![(RESOLVE_INGREDIENT_NAMES_STEP, Vec::new())];
    let mut previous_apply = None;
    for enrichment in enabled {
        let enrich_deps = enrichment
            .reads_enrichments()
            .iter()
            .filter(|read| enabled.contains(read))
            .map(|read| read.enrich_step_name())
            .collect();
        steps.push((enrichment.enrich_step_name(), enrich_deps));
        let apply_deps = std::iter::once(enrichment.enrich_step_name())
            .chain(previous_apply)
            .collect();
        steps.push((enrichment.apply_step_name(), apply_deps));
        previous_apply = Some(enrichment.apply_step_name());
    }
    steps
}

pub fn is_scrape_post_save_step(step_name: &str) -> bool {
    step_name == RESOLVE_INGREDIENT_NAMES_STEP
        || scrape_auto_applied_ai_step_names().contains(&step_name)
}

pub fn first_scrape_auto_applied_ai_step_name() -> Option<&'static str> {
    SCRAPE_AUTO_APPLIED_AI_ENRICHMENTS
        .first()
        .and_then(|enrichment| enrichment.step_names().first().copied())
}

pub fn step_after_scrape_auto_applied_ai_step(step_name: &str) -> Option<&'static str> {
    let step_names = scrape_auto_applied_ai_step_names();
    step_names
        .iter()
        .position(|name| *name == step_name)
        .and_then(|idx| step_names.get(idx + 1).copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_save_steps_follow_canonical_order() {
        let post_save: Vec<_> = scrape_post_save_step_dependencies()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        let canonical = scrape_pipeline_step_names();
        let save_idx = canonical.iter().position(|s| *s == "save_recipe").unwrap();
        assert_eq!(post_save, canonical[save_idx + 1..].to_vec());
        assert!(post_save.iter().all(|s| is_scrape_post_save_step(s)));
        assert!(!is_scrape_post_save_step("save_recipe"));
    }

    #[test]
    fn post_save_dependencies_only_point_backwards() {
        // Canonical order must be a valid run order, so a retry that resumes
        // at a step can treat everything before it as done.
        let steps = scrape_post_save_step_dependencies();
        for (idx, (name, deps)) in steps.iter().enumerate() {
            for dep in deps {
                let dep_idx = steps.iter().position(|(n, _)| n == dep);
                assert!(
                    dep_idx.is_some_and(|d| d < idx),
                    "{name} depends on {dep}, which does not come before it"
                );
            }
        }
    }

    #[test]
    fn post_save_dependencies() {
        assert_eq!(
            scrape_post_save_step_dependencies(),
            vec![
                ("resolve_ingredient_names", vec![]),
                ("enrich_normalize_title", vec![]),
                ("apply_normalized_title", vec!["enrich_normalize_title"]),
                (
                    "enrich_generate_description",
                    vec!["enrich_normalize_title"]
                ),
                (
                    "apply_generated_description",
                    vec!["enrich_generate_description", "apply_normalized_title"]
                ),
                ("enrich_auto_tag", vec![]),
                (
                    "apply_auto_tags",
                    vec!["enrich_auto_tag", "apply_generated_description"]
                ),
            ]
        );
    }
}
