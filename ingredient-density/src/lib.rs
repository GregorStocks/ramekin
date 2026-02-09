//! Ingredient density lookup for volume-to-weight conversion.
//!
//! This crate provides density data (grams per US cup) for common cooking
//! ingredients, enabling conversion from volume measurements to weight.
//!
//! Data sources:
//! - USDA FoodData Central SR Legacy (public domain, CC0)
//! - Curated overrides with citations for specific ingredients
//!
//! # Example
//!
//! ```
//! use ingredient_density::{find_density, volume_to_cups, is_volume_unit};
//!
//! // Look up density for flour
//! if let Some(grams_per_cup) = find_density("all-purpose flour", None) {
//!     // Convert 2 cups to grams
//!     let cups = volume_to_cups(2.0, "cup").unwrap();
//!     let grams = cups * grams_per_cup;
//!     println!("2 cups flour = {grams}g");
//! }
//! ```

mod density_lookup;

pub use density_lookup::{
    find_density, is_volume_unit, volume_to_cups, CUPS_PER_FL_OZ, CUPS_PER_GALLON, CUPS_PER_L,
    CUPS_PER_ML, CUPS_PER_PINT, CUPS_PER_QUART, CUPS_PER_TBSP, CUPS_PER_TSP,
};
