pub mod ai;
pub mod error;
pub mod extract;
pub mod fetch;
pub mod http;
pub mod image;
pub mod ingredient_parser;
pub mod metric_weights;
pub mod pipeline;
pub mod types;
pub mod volume_to_weight;

pub use error::{ExtractError, FetchError};
pub use extract::{extract_recipe, extract_recipe_with_stats};
pub use fetch::{fetch_bytes, fetch_html};
pub use http::{
    CacheStats, CachingClient, CachingClientBuilder, DiskCache, HttpClient, MockClient,
    MockResponse,
};
pub use image::{fetch_and_validate_image, validate_image, FetchedImage, MAX_FILE_SIZE};
pub use types::{
    EnrichAutoTagOutput, EnrichGeneratePhotoOutput, EnrichNormalizeIngredientsOutput,
    ExtractRecipeOutput, ExtractionAttempt, ExtractionMethod, FailedImageFetch, FetchHtmlOutput,
    FetchImagesOutput, ParseIngredientsOutput, PipelineStep, RawRecipe, SaveRecipeOutput,
    StepOutput,
};

/// Unique identifier for this build, generated at compile time.
/// Used to detect stale pipeline step outputs.
pub const BUILD_ID: &str = env!("BUILD_ID");
