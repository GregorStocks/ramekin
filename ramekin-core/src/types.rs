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
    EnrichNormalizeIngredients,
    EnrichAutoTag,
    EnrichGeneratePhoto,
}

impl PipelineStep {
    /// All steps in execution order
    pub const ALL: &'static [PipelineStep] = &[
        PipelineStep::FetchHtml,
        PipelineStep::ExtractRecipe,
        PipelineStep::FetchImages,
        PipelineStep::ParseIngredients,
        PipelineStep::SaveRecipe,
        PipelineStep::EnrichNormalizeIngredients,
        PipelineStep::EnrichAutoTag,
        PipelineStep::EnrichGeneratePhoto,
    ];

    /// Steps that should continue on failure (don't fail the overall job)
    pub fn continues_on_failure(&self) -> bool {
        matches!(
            self,
            PipelineStep::EnrichNormalizeIngredients
                | PipelineStep::EnrichAutoTag
                | PipelineStep::EnrichGeneratePhoto
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
            PipelineStep::EnrichNormalizeIngredients => "enrich_normalize_ingredients",
            PipelineStep::EnrichAutoTag => "enrich_auto_tag",
            PipelineStep::EnrichGeneratePhoto => "enrich_generate_photo",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "fetch_html" => Some(PipelineStep::FetchHtml),
            "extract_recipe" => Some(PipelineStep::ExtractRecipe),
            "fetch_images" => Some(PipelineStep::FetchImages),
            "parse_ingredients" => Some(PipelineStep::ParseIngredients),
            "save_recipe" => Some(PipelineStep::SaveRecipe),
            "enrich_normalize_ingredients" => Some(PipelineStep::EnrichNormalizeIngredients),
            "enrich_auto_tag" => Some(PipelineStep::EnrichAutoTag),
            "enrich_generate_photo" => Some(PipelineStep::EnrichGeneratePhoto),
            _ => None,
        }
    }
}

/// Output from the enrich_normalize_ingredients step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichNormalizeIngredientsOutput {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Output from the enrich_auto_tag step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichAutoTagOutput {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Output from the enrich_generate_photo step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichGeneratePhotoOutput {
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
}

/// Result of attempting a single extraction method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionAttempt {
    pub method: ExtractionMethod,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Recipe extracted from a page - fields are raw blobs, not parsed.
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
    pub source_url: String,
    pub source_name: Option<String>,
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
}
