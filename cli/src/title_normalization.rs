//! CLI command: generate title-normalization mapping from a .paprikarecipes file.
//!
//! Dumps recipe titles to a .txt (one per line) and produces a mapping file
//! (`Original -> Normalized`) by calling the title-normalize AI helper.
//! Cached AI responses make regeneration free.

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use ramekin_core::ai::{normalize_title, CachingAiClient};
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

/// A recipe with the fields we care about for title normalization.
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

/// Generate the titles input file and a normalized-titles output file.
///
/// - `paprika_file`: path to a `.paprikarecipes` archive.
/// - `titles_file`: where to write one recipe title per line (input).
/// - `output_file`: where to write `Original -> Normalized` mapping (output).
/// - `limit`: optional cap on number of recipes processed (default 500).
pub async fn run(
    paprika_file: &Path,
    titles_file: &Path,
    output_file: &Path,
    limit: Option<usize>,
) -> Result<()> {
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

    if let Some(parent) = titles_file.parent() {
        fs::create_dir_all(parent)?;
    }
    let titles_blob = entries
        .iter()
        .map(|e| e.title.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(titles_file, titles_blob)
        .with_context(|| format!("Failed to write {}", titles_file.display()))?;
    tracing::info!(
        "Wrote {} titles to {}",
        entries.len(),
        titles_file.display()
    );

    let ai = CachingAiClient::from_env()
        .context("Failed to build AI client (set OPENROUTER_API_KEY)")?;

    let mut lines = Vec::with_capacity(entries.len());
    let mut cached_hits = 0usize;
    for (idx, entry) in entries.iter().enumerate() {
        let result = normalize_title(&ai, &entry.title, &entry.ingredients, &entry.instructions)
            .await
            .with_context(|| format!("normalize_title failed for: {}", entry.title))?;
        if result.cached {
            cached_hits += 1;
        }
        let normalized = result.normalized_title.trim();
        lines.push(format!("{} -> {}", entry.title, normalized));
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
        "Wrote {} normalized titles to {} ({} cached, {} called AI)",
        lines.len(),
        output_file.display(),
        cached_hits,
        lines.len() - cached_hits
    );

    Ok(())
}
