//! Server-specific pipeline step implementations.
//!
//! These steps need database access and are implemented here rather than
//! in ramekin-core.

// Infrastructure for future generic pipeline integration.
// Currently unused while existing scraping code remains in use.
#![allow(dead_code)]
#![allow(unused_imports)]

mod fetch_images;
mod save_recipe;

pub use fetch_images::FetchImagesStep;
pub use save_recipe::SaveRecipeStep;
