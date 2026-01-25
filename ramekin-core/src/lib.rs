pub mod error;
pub mod extract;
pub mod fetch;
pub mod http;
pub mod pipeline;
pub mod types;

pub use error::{ExtractError, FetchError};
pub use extract::{extract_recipe, extract_recipe_with_stats};
pub use fetch::{fetch_bytes, fetch_html};
pub use http::{
    CacheStats, CachingClient, CachingClientBuilder, DiskCache, HttpClient, MockClient,
    MockResponse,
};
pub use types::{
    EnrichOutput, ExtractRecipeOutput, ExtractionAttempt, ExtractionMethod, FailedImageFetch,
    FetchHtmlOutput, FetchImagesOutput, PipelineStep, RawRecipe, SaveRecipeOutput, StepOutput,
};

/// Unique identifier for this build, generated at compile time.
/// Used to detect stale pipeline step outputs.
pub const BUILD_ID: &str = env!("BUILD_ID");
