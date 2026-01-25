//! CLI-specific pipeline step implementations.

mod fetch_images;
mod save_recipe;

pub use fetch_images::FetchImagesNoOp;
pub use save_recipe::SaveRecipeStep;
