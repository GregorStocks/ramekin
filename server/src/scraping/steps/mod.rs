//! Server-specific pipeline step implementations.
//!
//! These implement the `PipelineStep` trait with database operations
//! for storing recipes, fetching images, etc.

mod apply_auto_tags;
mod apply_generated_description;
mod apply_normalized_title;
mod fetch_html;
mod fetch_images;
mod helpers;
mod save_recipe;

pub use apply_auto_tags::ApplyAutoTagsStep;
pub use apply_generated_description::ApplyGeneratedDescriptionStep;
pub use apply_normalized_title::ApplyNormalizedTitleStep;
pub use fetch_html::FetchHtmlStep;
pub use fetch_images::FetchImagesStep;
pub use save_recipe::SaveRecipeStep;

// Enrich steps use generic implementations from ramekin-core.
