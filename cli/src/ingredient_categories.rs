//! Generate ingredient categories audit file from recipe fixtures.

use anyhow::Result;
use ramekin_core::ingredient_categorizer;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Fixture file structure (matches pipeline fixture format)
#[derive(Deserialize)]
struct FixtureFile {
    ingredients: Vec<IngredientFixture>,
}

#[derive(Deserialize)]
struct IngredientFixture {
    expected: ExpectedParsing,
}

#[derive(Deserialize)]
struct ExpectedParsing {
    item: String,
}

/// Generate a CSV file mapping all unique ingredients from fixtures to their categories.
pub fn generate(fixtures_dir: Option<&Path>, output: &Path) -> Result<()> {
    let fixtures_dir =
        fixtures_dir.unwrap_or(Path::new("ramekin-core/tests/fixtures/ingredient_parsing"));

    // Collect all unique ingredients from all fixture files
    let mut ingredients: BTreeMap<String, String> = BTreeMap::new();

    // Process pipeline fixtures
    let pipeline_dir = fixtures_dir.join("pipeline");
    if pipeline_dir.exists() {
        process_directory(&pipeline_dir, &mut ingredients)?;
    }

    // Process paprika fixtures
    let paprika_dir = fixtures_dir.join("paprika");
    if paprika_dir.exists() {
        process_directory(&paprika_dir, &mut ingredients)?;
    }

    // Process curated fixtures
    let curated_dir = fixtures_dir.join("curated");
    if curated_dir.exists() {
        process_directory(&curated_dir, &mut ingredients)?;
    }

    // Write output CSV
    let mut output_file = fs::File::create(output)?;
    writeln!(output_file, "ingredient,category")?;

    for (ingredient, category) in &ingredients {
        // Escape CSV fields that contain commas or quotes
        let escaped_ingredient = if ingredient.contains(',') || ingredient.contains('"') {
            format!("\"{}\"", ingredient.replace('"', "\"\""))
        } else {
            ingredient.clone()
        };
        writeln!(output_file, "{},{}", escaped_ingredient, category)?;
    }

    println!(
        "Generated {} ingredient categories to {}",
        ingredients.len(),
        output.display()
    );

    // Print summary by category
    let mut by_category: BTreeMap<&str, usize> = BTreeMap::new();
    for category in ingredients.values() {
        *by_category.entry(category.as_str()).or_default() += 1;
    }
    println!("\nCategories:");
    for (category, count) in by_category {
        println!("  {}: {}", category, count);
    }

    Ok(())
}

fn process_directory(dir: &Path, ingredients: &mut BTreeMap<String, String>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().is_some_and(|ext| ext == "json") {
            if let Err(e) = process_file(&path, ingredients) {
                eprintln!("Warning: failed to process {}: {}", path.display(), e);
            }
        }
    }
    Ok(())
}

fn process_file(path: &Path, ingredients: &mut BTreeMap<String, String>) -> Result<()> {
    let content = fs::read_to_string(path)?;
    let fixture: FixtureFile = serde_json::from_str(&content)?;

    for ingredient in fixture.ingredients {
        let item = ingredient.expected.item.trim().to_string();
        if !item.is_empty() && !ingredients.contains_key(&item) {
            let category = ingredient_categorizer::categorize(&item).to_string();
            ingredients.insert(item, category);
        }
    }

    Ok(())
}
