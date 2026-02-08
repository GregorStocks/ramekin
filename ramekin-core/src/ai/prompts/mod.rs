//! AI prompt templates.

pub mod auto_tag;
pub mod custom_enrich;
pub mod photo_extract;

pub use auto_tag::render_auto_tag_prompt;
pub use custom_enrich::{render_custom_enrich_system_prompt, render_custom_enrich_user_prompt};
pub use photo_extract::render_photo_extract_prompt;
