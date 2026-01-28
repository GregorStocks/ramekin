//! Pipeline step implementations.
//!
//! Generic steps are fully implemented here. DB-specific steps only have metadata.

mod enrich_auto_tag;
mod enrich_generate_photo;
mod enrich_normalize_ingredients;
mod extract_recipe;
mod fetch_html;
mod fetch_images;
mod parse_ingredients;
mod save_recipe;

pub use enrich_auto_tag::EnrichAutoTagStep;
pub use enrich_generate_photo::EnrichGeneratePhotoStep;
pub use enrich_normalize_ingredients::EnrichNormalizeIngredientsStep;
pub use extract_recipe::ExtractRecipeStep;
pub use fetch_html::FetchHtmlStep;
pub use fetch_images::FetchImagesStepMeta;
pub use parse_ingredients::ParseIngredientsStep;
pub use save_recipe::SaveRecipeStepMeta;
