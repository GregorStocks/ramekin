use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Parse a recipe from an HTML file.
/// Outputs JSON to stdout (either RawRecipe on success or error message on failure).
pub fn parse_html(file: &Path, source_url: &str) -> Result<()> {
    let html = fs::read_to_string(file)
        .with_context(|| format!("Failed to read HTML file: {}", file.display()))?;

    match ramekin_core::extract_recipe(&html, source_url) {
        Ok(recipe) => {
            let json = serde_json::to_string_pretty(&recipe)?;
            println!("{}", json);
            Ok(())
        }
        Err(e) => {
            let error_json = serde_json::json!({
                "error": e.to_string()
            });
            println!("{}", serde_json::to_string_pretty(&error_json)?);
            // Return error so exit code is non-zero
            Err(anyhow::anyhow!("Failed to extract recipe: {}", e))
        }
    }
}
