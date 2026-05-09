use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Pipeline steps in execution order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStep {
    FetchHtml,
    ExtractRecipe,
    FetchImages,
    ParseIngredients,
    SaveRecipe,
    EnrichNormalizeTitle,
    ApplyNormalizedTitle,
    EnrichGenerateDescription,
    ApplyGeneratedDescription,
    EnrichAutoTag,
    ApplyAutoTags,
}

impl PipelineStep {
    /// All steps in execution order
    pub const ALL: &'static [PipelineStep] = &[
        PipelineStep::FetchHtml,
        PipelineStep::ExtractRecipe,
        PipelineStep::FetchImages,
        PipelineStep::ParseIngredients,
        PipelineStep::SaveRecipe,
        PipelineStep::EnrichNormalizeTitle,
        PipelineStep::ApplyNormalizedTitle,
        PipelineStep::EnrichGenerateDescription,
        PipelineStep::ApplyGeneratedDescription,
        PipelineStep::EnrichAutoTag,
        PipelineStep::ApplyAutoTags,
    ];

    /// Steps that should continue on failure (don't fail the overall job)
    pub fn continues_on_failure(&self) -> bool {
        matches!(
            self,
            PipelineStep::EnrichAutoTag | PipelineStep::ApplyAutoTags
        )
    }

    /// Steps that are DB-specific (CLI can skip or stub these)
    pub fn is_db_specific(&self) -> bool {
        matches!(self, PipelineStep::FetchImages)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PipelineStep::FetchHtml => "fetch_html",
            PipelineStep::ExtractRecipe => "extract_recipe",
            PipelineStep::FetchImages => "fetch_images",
            PipelineStep::ParseIngredients => "parse_ingredients",
            PipelineStep::SaveRecipe => "save_recipe",
            PipelineStep::EnrichNormalizeTitle => "enrich_normalize_title",
            PipelineStep::ApplyNormalizedTitle => "apply_normalized_title",
            PipelineStep::EnrichGenerateDescription => "enrich_generate_description",
            PipelineStep::ApplyGeneratedDescription => "apply_generated_description",
            PipelineStep::EnrichAutoTag => "enrich_auto_tag",
            PipelineStep::ApplyAutoTags => "apply_auto_tags",
        }
    }

    #[allow(clippy::should_implement_trait)] // Returns Option, not Result
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "fetch_html" => Some(PipelineStep::FetchHtml),
            "extract_recipe" => Some(PipelineStep::ExtractRecipe),
            "fetch_images" => Some(PipelineStep::FetchImages),
            "parse_ingredients" => Some(PipelineStep::ParseIngredients),
            "save_recipe" => Some(PipelineStep::SaveRecipe),
            "enrich_normalize_title" => Some(PipelineStep::EnrichNormalizeTitle),
            "apply_normalized_title" => Some(PipelineStep::ApplyNormalizedTitle),
            "enrich_generate_description" => Some(PipelineStep::EnrichGenerateDescription),
            "apply_generated_description" => Some(PipelineStep::ApplyGeneratedDescription),
            "enrich_auto_tag" => Some(PipelineStep::EnrichAutoTag),
            "apply_auto_tags" => Some(PipelineStep::ApplyAutoTags),
            _ => None,
        }
    }
}

/// Output from the enrich_auto_tag step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichAutoTagOutput {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Identifies which extraction method was used
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionMethod {
    JsonLd,
    Microdata,
    /// Structured data supplemented with HTML class-based fallbacks
    HtmlFallback,
    /// Imported from Paprika app
    Paprika,
    /// Extracted from uploaded photos using vision AI
    PhotoUpload,
}

/// Result of attempting a single extraction method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionAttempt {
    pub method: ExtractionMethod,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Recipe extracted from a page or imported - fields are raw blobs, not parsed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawRecipe {
    pub title: String,
    pub description: Option<String>,
    /// Ingredients as a newline-separated blob
    pub ingredients: String,
    /// Instructions as a blob (could be HTML or plain text)
    pub instructions: String,
    /// Image URLs found in the recipe (not yet fetched)
    pub image_urls: Vec<String>,
    /// Source URL (optional for imports that don't have a web source)
    pub source_url: Option<String>,
    pub source_name: Option<String>,
    /// Servings (e.g., "4 servings", "6-8")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servings: Option<String>,
    /// Prep time (e.g., "15 minutes")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prep_time: Option<String>,
    /// Cook time (e.g., "30 minutes")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cook_time: Option<String>,
    /// Total time (e.g., "45 minutes")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_time: Option<String>,
    /// Rating (1-5)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<i32>,
    /// Difficulty level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<String>,
    /// Nutritional information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nutritional_info: Option<String>,
    /// Additional notes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Categories/tags from import source
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
    /// Footnote text extracted from HTML, keyed by marker (e.g., "*", "**")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footnotes: Option<Vec<(String, String)>>,
}

/// Output from a pipeline step, stored in step_data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepOutput<T> {
    pub build_id: String,
    pub output: T,
    pub next_step: Option<String>,
}

/// Output from the fetch_html step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchHtmlOutput {
    pub html: String,
}

/// Output from the extract_recipe step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractRecipeOutput {
    pub raw_recipe: RawRecipe,
    /// Which method was used to extract the recipe
    pub method_used: ExtractionMethod,
    /// Results from all attempted extraction methods
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub all_attempts: Vec<ExtractionAttempt>,
}

/// Output from the save_recipe step (for disk-based pipeline testing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveRecipeOutput {
    pub raw_recipe: RawRecipe,
    pub saved_at: String,
}

/// Output from the fetch_images step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchImagesOutput {
    /// Successfully downloaded photo IDs
    pub photo_ids: Vec<Uuid>,
    /// URLs that failed to download, with error messages
    pub failed_urls: Vec<FailedImageFetch>,
}

/// A failed image fetch attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedImageFetch {
    pub url: String,
    pub error: String,
}

/// Output from the parse_ingredients step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseIngredientsOutput {
    pub ingredients: Vec<crate::ingredient_parser::ParsedIngredient>,
    /// Statistics about volume-to-weight conversion
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume_stats: Option<crate::volume_to_weight::VolumeConversionStats>,
    /// Statistics about metric weight conversion
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric_stats: Option<crate::metric_weights::MetricConversionStats>,
}
