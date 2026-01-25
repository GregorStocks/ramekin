//! Pipeline step implementations.
//!
//! Generic steps are fully implemented here. DB-specific steps only have metadata.

mod enrich;
mod extract_recipe;
mod fetch_html;
mod fetch_images;
mod save_recipe;

pub use enrich::EnrichStep;
pub use extract_recipe::ExtractRecipeStep;
pub use fetch_html::FetchHtmlStep;
pub use fetch_images::FetchImagesStepMeta;
pub use save_recipe::SaveRecipeStepMeta;
