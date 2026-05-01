//! Single source of truth for AI enrichments automatically applied by scraping.

/// AI enrichments that scrape jobs should run and apply without user review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrapeAutoAppliedAiEnrichment {
    AutoTag,
}

const CORE_SCRAPE_STEP_NAMES: &[&str] = &[
    "fetch_html",
    "extract_recipe",
    "fetch_images",
    "parse_ingredients",
    "save_recipe",
];

const AUTO_TAG_STEP_NAMES: &[&str] = &["enrich_auto_tag", "apply_auto_tags"];

/// Add to this list when a new AI enrichment should be auto-applied at scrape time.
pub const SCRAPE_AUTO_APPLIED_AI_ENRICHMENTS: &[ScrapeAutoAppliedAiEnrichment] =
    &[ScrapeAutoAppliedAiEnrichment::AutoTag];

impl ScrapeAutoAppliedAiEnrichment {
    pub fn step_names(self) -> &'static [&'static str] {
        match self {
            ScrapeAutoAppliedAiEnrichment::AutoTag => AUTO_TAG_STEP_NAMES,
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
