use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
