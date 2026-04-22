//! CLI command: generate description mapping from a .paprikarecipes file.
//!
//! Reads recipes from a `.paprikarecipes` archive, generates a concise
//! menu-style description for each via the AI, and writes a mapping file
//! (`Title -> Description`). Cached AI responses make regeneration free.

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use ramekin_core::ai::{generate_description, CachingAiClient};
use serde::Deserialize;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

#[derive(Debug, Deserialize)]
struct PaprikaRecipe {
    name: String,
    #[serde(default)]
    ingredients: Option<String>,
    #[serde(default)]
    directions: Option<String>,
}

/// A recipe with the fields we care about for description generation.
struct PaprikaEntry {
    title: String,
    ingredients: String,
    instructions: String,
}

/// Read all recipes from a `.paprikarecipes` archive in archive order.
fn read_paprika_entries(paprika_file: &Path) -> Result<Vec<PaprikaEntry>> {
    let file = File::open(paprika_file)
        .with_context(|| format!("Failed to open file: {}", paprika_file.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("Failed to read zip archive: {}", paprika_file.display()))?;

    let mut entries = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let entry_name = entry.name().to_string();
        if !entry_name.ends_with(".paprikarecipe") {
            continue;
        }

        let mut compressed = Vec::new();
        entry.read_to_end(&mut compressed)?;

        let mut decoder = GzDecoder::new(&compressed[..]);
        let mut json = String::new();
        decoder
            .read_to_string(&mut json)
            .with_context(|| format!("Failed to decompress recipe: {}", entry_name))?;

        let recipe: PaprikaRecipe = serde_json::from_str(&json)
            .with_context(|| format!("Failed to parse recipe JSON: {}", entry_name))?;

        let title = recipe.name.trim().to_string();
        if title.is_empty() {
            continue;
        }
        entries.push(PaprikaEntry {
            title,
            ingredients: recipe.ingredients.unwrap_or_default(),
            instructions: recipe.directions.unwrap_or_default(),
        });
    }

    Ok(entries)
}

/// Generate description mapping from a `.paprikarecipes` file.
///
/// - `paprika_file`: path to a `.paprikarecipes` archive.
/// - `output_file`: where to write `Title -> Description` mapping (output).
/// - `limit`: optional cap on number of recipes processed (default 500).
pub async fn run(paprika_file: &Path, output_file: &Path, limit: Option<usize>) -> Result<()> {
    let all_entries = read_paprika_entries(paprika_file)?;
    let limit = limit.unwrap_or(500);

    // Deduplicate by title preserving order, then cap at `limit`.
    let mut seen = std::collections::HashSet::new();
    let mut entries: Vec<PaprikaEntry> = Vec::new();
    for e in all_entries {
        if seen.insert(e.title.clone()) {
            entries.push(e);
            if entries.len() >= limit {
                break;
            }
        }
    }

    let ai = CachingAiClient::from_env()
        .context("Failed to build AI client (set OPENROUTER_API_KEY)")?;

    let mut lines = Vec::with_capacity(entries.len());
    let mut cached_hits = 0usize;
    for (idx, entry) in entries.iter().enumerate() {
        let result =
            generate_description(&ai, &entry.title, &entry.ingredients, &entry.instructions)
                .await
                .with_context(|| format!("generate_description failed for: {}", entry.title))?;
        if result.cached {
            cached_hits += 1;
        }
        let description = result.description.trim();
        lines.push(format!("{} -> {}", entry.title, description));
        if (idx + 1) % 50 == 0 {
            tracing::info!(
                "  processed {}/{} ({} cached)",
                idx + 1,
                entries.len(),
                cached_hits
            );
        }
    }

    if let Some(parent) = output_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_file, lines.join("\n") + "\n")
        .with_context(|| format!("Failed to write {}", output_file.display()))?;

    tracing::info!(
        "Wrote {} descriptions to {} ({} cached, {} called AI)",
        lines.len(),
        output_file.display(),
        cached_hits,
        lines.len() - cached_hits
    );

    Ok(())
}
