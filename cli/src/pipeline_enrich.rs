//! Enrichment test harness.
//!
//! Tests enrichments against recipes from pipeline results.

use crate::pipeline_orchestrator::{FinalStatus, PipelineResults};
use anyhow::{Context, Result};
use chrono::Utc;
use ramekin_client::apis::configuration::Configuration;
use ramekin_client::apis::enrich_api;
use ramekin_client::models::{EnrichRequest, EnrichmentInfo, Ingredient, RecipeContent};
use ramekin_core::{RawRecipe, SaveRecipeOutput};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

// ============================================================================
// Configuration
// ============================================================================

pub struct PipelineEnrichConfig {
    pub server_url: String,
    pub auth_token: String,
    pub runs_dir: PathBuf,
    pub output_dir: PathBuf,
    pub limit: Option<usize>,
    pub enrichment_type: Option<String>,
    pub site_filter: Option<String>,
}

// ============================================================================
// Results
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PipelineEnrichResults {
    pub total_recipes: usize,
    pub total_enrichments: usize,
    pub successful: usize,
    pub failed: usize,
    pub unchanged: usize,
    pub by_enrichment_type: HashMap<String, EnrichmentTypeResults>,
    pub recipe_results: Vec<RecipeEnrichResult>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnrichmentTypeResults {
    pub enrichment_type: String,
    pub display_name: String,
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub unchanged: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeEnrichResult {
    pub url: String,
    pub title: String,
    pub enrichment_type: String,
    pub success: bool,
    pub changed: bool,
    pub duration_ms: u64,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<serde_json::Value>,
}

// ============================================================================
// Main test runner
// ============================================================================

pub async fn run_pipeline_enrich(config: PipelineEnrichConfig) -> Result<PipelineEnrichResults> {
    // Generate run ID
    let now = Utc::now();
    let run_id = now.format("enrich-%Y-%m-%d_%H-%M-%S").to_string();
    let run_dir = config.output_dir.join(&run_id);
    fs::create_dir_all(&run_dir)?;

    // Load pipeline results
    let (pipeline_run_id, pipeline_results) = load_latest_pipeline_results(&config.runs_dir)?;

    // Collect recipes that completed extraction
    let recipes = collect_recipes(&config.runs_dir, &pipeline_run_id, &pipeline_results)?;
    tracing::info!(
        "Loaded {} recipes from pipeline run {}",
        recipes.len(),
        pipeline_run_id
    );

    // Apply filters
    let mut recipes = recipes;
    if let Some(ref site) = config.site_filter {
        recipes.retain(|(url, _)| url.contains(site));
    }
    if let Some(limit) = config.limit {
        recipes.truncate(limit);
    }

    // Set up API client
    let mut api_config = Configuration::new();
    api_config.base_path = config.server_url.clone();
    api_config.bearer_access_token = Some(config.auth_token.clone());

    // Get available enrichment types
    let enrichments = enrich_api::list_enrichments(&api_config)
        .await
        .context("Failed to list enrichments")?;

    let enrichment_types: Vec<&EnrichmentInfo> = match &config.enrichment_type {
        Some(filter) => enrichments
            .enrichments
            .iter()
            .filter(|e| e.r#type == *filter)
            .collect(),
        None => enrichments.enrichments.iter().collect(),
    };

    if enrichment_types.is_empty() {
        anyhow::bail!("No matching enrichment types found");
    }

    // Initialize results
    let mut results = PipelineEnrichResults {
        total_recipes: recipes.len(),
        total_enrichments: recipes.len() * enrichment_types.len(),
        ..Default::default()
    };

    for enrichment in &enrichment_types {
        results.by_enrichment_type.insert(
            enrichment.r#type.clone(),
            EnrichmentTypeResults {
                enrichment_type: enrichment.r#type.clone(),
                display_name: enrichment.display_name.clone(),
                ..Default::default()
            },
        );
    }

    println!("Enrichment Test Starting");
    println!("========================");
    println!("Run ID: {}", run_id);
    println!("Recipes: {}", recipes.len());
    println!(
        "Enrichment types: {}",
        enrichment_types
            .iter()
            .map(|e| e.r#type.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!();

    let total = recipes.len() * enrichment_types.len();
    let mut processed = 0;

    // Process each recipe × enrichment type
    for (url, raw_recipe) in &recipes {
        let recipe_content = raw_recipe_to_content(raw_recipe);

        for enrichment in &enrichment_types {
            processed += 1;
            print!(
                "[{}/{}] {} × {} ... ",
                processed,
                total,
                truncate(&raw_recipe.title, 30),
                enrichment.r#type
            );

            let start = Instant::now();
            let result = run_single_enrichment(
                &api_config,
                url,
                &raw_recipe.title,
                &recipe_content,
                enrichment,
            )
            .await;
            let duration_ms = start.elapsed().as_millis() as u64;

            let recipe_result = match result {
                Ok((enriched, changed)) => {
                    if changed {
                        println!("changed ({} ms)", duration_ms);
                    } else {
                        println!("- unchanged ({} ms)", duration_ms);
                    }
                    RecipeEnrichResult {
                        url: url.clone(),
                        title: raw_recipe.title.clone(),
                        enrichment_type: enrichment.r#type.clone(),
                        success: true,
                        changed,
                        duration_ms,
                        error: None,
                        before: if changed {
                            Some(extract_fields(&recipe_content, &enrichment.output_fields))
                        } else {
                            None
                        },
                        after: if changed {
                            Some(extract_fields(&enriched, &enrichment.output_fields))
                        } else {
                            None
                        },
                    }
                }
                Err(e) => {
                    println!("X error: {}", e);
                    RecipeEnrichResult {
                        url: url.clone(),
                        title: raw_recipe.title.clone(),
                        enrichment_type: enrichment.r#type.clone(),
                        success: false,
                        changed: false,
                        duration_ms,
                        error: Some(e.to_string()),
                        before: None,
                        after: None,
                    }
                }
            };

            // Update stats
            if recipe_result.success {
                results.successful += 1;
                if recipe_result.changed {
                    // Changed
                } else {
                    results.unchanged += 1;
                }
            } else {
                results.failed += 1;
            }

            if let Some(type_results) = results.by_enrichment_type.get_mut(&enrichment.r#type) {
                type_results.total += 1;
                if recipe_result.success {
                    type_results.successful += 1;
                    if !recipe_result.changed {
                        type_results.unchanged += 1;
                    }
                } else {
                    type_results.failed += 1;
                }
            }

            results.recipe_results.push(recipe_result);
        }
    }

    // Save results
    let results_json = serde_json::to_string_pretty(&results)?;
    fs::write(run_dir.join("results.json"), &results_json)?;

    // Generate and save report
    let report = generate_report(&results);
    fs::write(run_dir.join("report.md"), &report)?;

    // Print summary
    println!();
    println!("Enrichment Test Results");
    println!("=======================");
    println!("Run ID: {}", run_id);
    println!("Total enrichments: {}", results.total_enrichments);
    println!(
        "Successful: {} ({:.1}%)",
        results.successful,
        pct(results.successful, results.total_enrichments)
    );
    println!(
        "Failed: {} ({:.1}%)",
        results.failed,
        pct(results.failed, results.total_enrichments)
    );
    println!(
        "Unchanged: {} ({:.1}%)",
        results.unchanged,
        pct(results.unchanged, results.total_enrichments)
    );
    println!();
    println!("Results saved to: {}", run_dir.display());

    Ok(results)
}

// ============================================================================
// Helper functions
// ============================================================================

async fn run_single_enrichment(
    config: &Configuration,
    _url: &str,
    _title: &str,
    recipe: &RecipeContent,
    enrichment: &EnrichmentInfo,
) -> Result<(RecipeContent, bool)> {
    let request = EnrichRequest {
        enrichment_type: enrichment.r#type.clone(),
        recipe: Box::new(recipe.clone()),
    };

    let enriched = enrich_api::enrich_recipe(config, request)
        .await
        .map_err(|e| anyhow::anyhow!("API error: {:?}", e))?;

    // Check if the relevant fields changed
    let changed = fields_changed(recipe, &enriched, &enrichment.output_fields);

    Ok((enriched, changed))
}

fn fields_changed(before: &RecipeContent, after: &RecipeContent, fields: &[String]) -> bool {
    for field in fields {
        match field.as_str() {
            "ingredients" => {
                if before.ingredients != after.ingredients {
                    return true;
                }
            }
            "instructions" => {
                if before.instructions != after.instructions {
                    return true;
                }
            }
            "prep_time" | "prepTime" => {
                if before.prep_time != after.prep_time {
                    return true;
                }
            }
            "cook_time" | "cookTime" => {
                if before.cook_time != after.cook_time {
                    return true;
                }
            }
            "total_time" | "totalTime" => {
                if before.total_time != after.total_time {
                    return true;
                }
            }
            "nutritional_info" | "nutritionalInfo" => {
                if before.nutritional_info != after.nutritional_info {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn extract_fields(recipe: &RecipeContent, fields: &[String]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for field in fields {
        match field.as_str() {
            "ingredients" => {
                map.insert(
                    "ingredients".to_string(),
                    serde_json::to_value(&recipe.ingredients).unwrap_or_default(),
                );
            }
            "instructions" => {
                map.insert(
                    "instructions".to_string(),
                    serde_json::Value::String(recipe.instructions.clone()),
                );
            }
            "prep_time" | "prepTime" => {
                map.insert(
                    "prep_time".to_string(),
                    serde_json::to_value(&recipe.prep_time).unwrap_or_default(),
                );
            }
            "cook_time" | "cookTime" => {
                map.insert(
                    "cook_time".to_string(),
                    serde_json::to_value(&recipe.cook_time).unwrap_or_default(),
                );
            }
            "total_time" | "totalTime" => {
                map.insert(
                    "total_time".to_string(),
                    serde_json::to_value(&recipe.total_time).unwrap_or_default(),
                );
            }
            "nutritional_info" | "nutritionalInfo" => {
                map.insert(
                    "nutritional_info".to_string(),
                    serde_json::to_value(&recipe.nutritional_info).unwrap_or_default(),
                );
            }
            _ => {}
        }
    }
    serde_json::Value::Object(map)
}

fn raw_recipe_to_content(raw: &RawRecipe) -> RecipeContent {
    // Parse ingredients from newline-separated string into structured format
    let ingredients: Vec<Ingredient> = raw
        .ingredients
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| Ingredient {
            item: line.trim().to_string(),
            amount: None,
            unit: None,
            note: None,
        })
        .collect();

    RecipeContent {
        title: raw.title.clone(),
        description: raw.description.clone().map(Some),
        ingredients,
        instructions: raw.instructions.clone(),
        source_url: Some(Some(raw.source_url.clone())),
        source_name: raw.source_name.clone().map(Some),
        tags: None,
        servings: None,
        prep_time: None,
        cook_time: None,
        total_time: None,
        rating: None,
        difficulty: None,
        nutritional_info: None,
        notes: None,
    }
}

fn load_latest_pipeline_results(runs_dir: &Path) -> Result<(String, PipelineResults)> {
    let mut runs: Vec<_> = fs::read_dir(runs_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| {
            // Skip enrich test runs
            !e.file_name().to_string_lossy().starts_with("enrich-")
        })
        .collect();

    runs.sort_by_key(|e| e.file_name());
    runs.reverse();

    let latest = runs
        .first()
        .ok_or_else(|| anyhow::anyhow!("No pipeline runs found in {}", runs_dir.display()))?;

    let run_id = latest.file_name().to_string_lossy().to_string();
    let results_path = latest.path().join("results.json");

    let content = fs::read_to_string(&results_path)
        .with_context(|| format!("Failed to read {}", results_path.display()))?;

    let results: PipelineResults = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", results_path.display()))?;

    Ok((run_id, results))
}

fn collect_recipes(
    runs_dir: &Path,
    run_id: &str,
    results: &PipelineResults,
) -> Result<Vec<(String, RawRecipe)>> {
    let mut recipes = Vec::new();

    for url_result in &results.url_results {
        if !matches!(url_result.final_status, FinalStatus::Completed) {
            continue;
        }

        // Load the saved recipe output
        let slug = ramekin_core::http::slugify_url(&url_result.url);
        let save_output_path = runs_dir
            .join(run_id)
            .join("urls")
            .join(&slug)
            .join("save_recipe")
            .join("output.json");

        if let Ok(content) = fs::read_to_string(&save_output_path) {
            if let Ok(output) = serde_json::from_str::<SaveRecipeOutput>(&content) {
                recipes.push((url_result.url.clone(), output.raw_recipe));
            }
        }
    }

    Ok(recipes)
}

fn generate_report(results: &PipelineEnrichResults) -> String {
    let mut report = String::new();

    report.push_str("# Enrichment Test Report\n\n");
    report.push_str("## Overall\n\n");
    report.push_str(&format!("- Total recipes: {}\n", results.total_recipes));
    report.push_str(&format!(
        "- Total enrichment runs: {}\n",
        results.total_enrichments
    ));
    report.push_str(&format!(
        "- Successful: {} ({:.1}%)\n",
        results.successful,
        pct(results.successful, results.total_enrichments)
    ));
    report.push_str(&format!(
        "- Failed: {} ({:.1}%)\n",
        results.failed,
        pct(results.failed, results.total_enrichments)
    ));
    report.push_str(&format!(
        "- Unchanged: {} ({:.1}%)\n",
        results.unchanged,
        pct(results.unchanged, results.total_enrichments)
    ));

    report.push_str("\n## By Enrichment Type\n\n");
    report.push_str("| Type | Success | Failed | Unchanged |\n");
    report.push_str("|------|---------|--------|------------|\n");

    let mut types: Vec<_> = results.by_enrichment_type.values().collect();
    types.sort_by_key(|t| &t.enrichment_type);

    for type_result in types {
        report.push_str(&format!(
            "| {} | {} ({:.1}%) | {} ({:.1}%) | {} ({:.1}%) |\n",
            type_result.display_name,
            type_result.successful,
            pct(type_result.successful, type_result.total),
            type_result.failed,
            pct(type_result.failed, type_result.total),
            type_result.unchanged,
            pct(type_result.unchanged, type_result.total),
        ));
    }

    // Failed enrichments
    let failures: Vec<_> = results
        .recipe_results
        .iter()
        .filter(|r| !r.success)
        .collect();

    if !failures.is_empty() {
        report.push_str("\n## Failed Enrichments\n\n");

        // Group by error message
        let mut by_error: std::collections::BTreeMap<String, Vec<&RecipeEnrichResult>> =
            std::collections::BTreeMap::new();
        for failure in &failures {
            let error = failure
                .error
                .as_ref()
                .map(|e| simplify_error(e))
                .unwrap_or_else(|| "Unknown error".to_string());
            by_error.entry(error).or_default().push(failure);
        }

        for (error, items) in by_error {
            report.push_str(&format!("\n### {} ({} items)\n\n", error, items.len()));
            for item in items.iter().take(5) {
                report.push_str(&format!("- {} ({})\n", item.title, item.enrichment_type));
            }
            if items.len() > 5 {
                report.push_str(&format!("- ... and {} more\n", items.len() - 5));
            }
        }
    }

    report
}

fn simplify_error(error: &str) -> String {
    if error.contains("rate limit") || error.contains("RateLimited") {
        "Rate limited".to_string()
    } else if error.contains("timeout") {
        "Timeout".to_string()
    } else if error.contains("503") || error.contains("unavailable") {
        "Service unavailable".to_string()
    } else {
        let truncated: String = error.chars().take(50).collect();
        if truncated.len() < error.len() {
            format!("{}...", truncated)
        } else {
            truncated
        }
    }
}

fn pct(num: usize, denom: usize) -> f64 {
    if denom == 0 {
        0.0
    } else {
        num as f64 / denom as f64 * 100.0
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}
