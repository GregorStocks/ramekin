//! Server-specific pipeline step implementations.
//!
//! These implement the `PipelineStep` trait with database operations
//! for storing recipes, fetching images, etc.

mod apply_normalize;
mod edit_with_ai;
mod extract_text;
mod fetch_html;
mod fetch_images;
mod helpers;
mod load_recipe;
mod propose_normalize;
mod resolve_ingredient_names;
mod save_recipe;

pub use apply_normalize::ApplyNormalizeStep;
pub use edit_with_ai::{EditRequest, EditWithAiStep};
pub use extract_text::ExtractTextStep;
pub use fetch_html::FetchHtmlStep;
pub use fetch_images::FetchImagesStep;
pub use load_recipe::LoadRecipeStep;
pub use propose_normalize::{NormalizeProposal, ProposeNormalizeStep};
pub use resolve_ingredient_names::ResolveIngredientNamesStep;
pub use save_recipe::SaveRecipeStep;

// Normalize steps use generic implementations from ramekin-core.
