//! Line filtering, prep/guidance notes, section headers, and item splitting.

mod line_classifiers;
mod normalize;
mod prep_notes;

pub use line_classifiers::{detect_section_header, should_ignore_line};
pub(in crate::ingredient_parser) use normalize::*;
pub(in crate::ingredient_parser) use prep_notes::*;

pub(super) fn split_compound_items(item: &str) -> Vec<String> {
    // Normalize Oxford commas
    let normalized = item.replace(", and ", ", ");

    let mut result = Vec::new();
    for part in normalized.split(", ") {
        for sub in part.split(" and ") {
            let trimmed = sub.trim();
            if !trimmed.is_empty() {
                result.push(trimmed.to_string());
            }
        }
    }

    result
}

#[cfg(test)]
mod tests;
