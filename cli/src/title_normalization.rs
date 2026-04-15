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
}

/// Read all recipe titles from a `.paprikarecipes` archive in archive order.
fn read_paprika_titles(paprika_file: &Path) -> Result<Vec<String>> {
    let file = File::open(paprika_file)
        .with_context(|| format!("Failed to open file: {}", paprika_file.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("Failed to read zip archive: {}", paprika_file.display()))?;

    let mut titles = Vec::new();
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

        let name = recipe.name.trim().to_string();
        if !name.is_empty() {
            titles.push(name);
        }
    }

    Ok(titles)
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
    let all_titles = read_paprika_titles(paprika_file)?;
    let limit = limit.unwrap_or(500);

    // Deduplicate preserving order, then cap at `limit`.
    let mut seen = std::collections::HashSet::new();
    let mut titles: Vec<String> = Vec::new();
    for t in all_titles {
        if seen.insert(t.clone()) {
            titles.push(t);
            if titles.len() >= limit {
                break;
            }
        }
    }

    if let Some(parent) = titles_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(titles_file, titles.join("\n") + "\n")
        .with_context(|| format!("Failed to write {}", titles_file.display()))?;
    println!("Wrote {} titles to {}", titles.len(), titles_file.display());

    let ai = CachingAiClient::from_env()
        .context("Failed to build AI client (set OPENROUTER_API_KEY or RAMEKIN_AI_OFFLINE=true)")?;

    let mut lines = Vec::with_capacity(titles.len());
    let mut cached_hits = 0usize;
    for (idx, title) in titles.iter().enumerate() {
        let result = normalize_title(&ai, title)
            .await
            .with_context(|| format!("normalize_title failed for: {}", title))?;
        if result.cached {
            cached_hits += 1;
        }
        let normalized = result.normalized_title.trim();
        lines.push(format!("{} -> {}", title, normalized));
        if (idx + 1) % 50 == 0 {
            println!(
                "  processed {}/{} ({} cached)",
                idx + 1,
                titles.len(),
                cached_hits
            );
        }
    }

    if let Some(parent) = output_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_file, lines.join("\n") + "\n")
        .with_context(|| format!("Failed to write {}", output_file.display()))?;

    println!(
        "Wrote {} normalized titles to {} ({} cached, {} called AI)",
        lines.len(),
        output_file.display(),
        cached_hits,
        lines.len() - cached_hits
    );

    Ok(())
}
