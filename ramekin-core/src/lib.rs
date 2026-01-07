pub mod error;
pub mod extract;
pub mod fetch;
pub mod types;

pub use error::{ExtractError, FetchError};
pub use extract::extract_recipe;
pub use fetch::fetch_html;
pub use types::{ExtractRecipeOutput, FetchHtmlOutput, RawRecipe, StepOutput};

/// Unique identifier for this build, generated at compile time.
/// Used to detect stale pipeline step outputs.
pub const BUILD_ID: &str = env!("BUILD_ID");
