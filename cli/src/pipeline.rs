use anyhow::{anyhow, Result};
use chrono::Utc;
use ramekin_core::enrichment::run_all_enrichments;
use ramekin_core::http::{CachingClient, HttpClient};
use ramekin_core::llm::LlmProvider;
use ramekin_core::{Ingredient, RawRecipe, RecipeContent};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

// ============================================================================
// Pipeline step enum
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStep {
    FetchHtml,
    ExtractRecipe,
    EnrichRecipe,
    SaveRecipe,
}

impl PipelineStep {
    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "fetch_html" => Ok(PipelineStep::FetchHtml),
            "extract_recipe" => Ok(PipelineStep::ExtractRecipe),
            "enrich_recipe" => Ok(PipelineStep::EnrichRecipe),
            "save_recipe" => Ok(PipelineStep::SaveRecipe),
            _ => Err(anyhow!(
                "Unknown step: {}. Valid steps: fetch_html, extract_recipe, enrich_recipe, save_recipe",
                s
            )),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PipelineStep::FetchHtml => "fetch_html",
            PipelineStep::ExtractRecipe => "extract_recipe",
            PipelineStep::EnrichRecipe => "enrich_recipe",
            PipelineStep::SaveRecipe => "save_recipe",
        }
    }
}

// ============================================================================
// Step result types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step: PipelineStep,
    pub success: bool,
    pub duration_ms: u64,
    pub error: Option<String>,
    pub cached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepTiming {
    pub started_at: String,
    pub completed_at: String,
    pub duration_ms: u64,
}

/// Stats about which extraction methods succeeded
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionStats {
    pub method_used: ramekin_core::ExtractionMethod,
    pub jsonld_success: bool,
    pub microdata_success: bool,
}

/// Result from the extract_recipe step with extraction stats
#[derive(Debug, Clone)]
pub struct ExtractStepResult {
    pub step_result: StepResult,
    pub extraction_stats: Option<ExtractionStats>,
}

/// Result from running all steps, including extraction and enrichment stats
#[derive(Debug, Clone)]
pub struct AllStepsResult {
    pub step_results: Vec<StepResult>,
    pub extraction_stats: Option<ExtractionStats>,
    /// Errors from enrichments that failed (enrichment_type, error_message)
    #[allow(dead_code)]
    pub enrichment_errors: Vec<(String, String)>,
}

// ============================================================================
// URL slugification (re-exported from ramekin_core)
// ============================================================================

pub use ramekin_core::http::slugify_url;

// ============================================================================
// Staging directory utilities
// ============================================================================

/// Get the staging directory path for manual HTML saves
pub fn staging_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".ramekin").join("cache-staging"))
        .unwrap_or_else(|| PathBuf::from("data/cache-staging"))
}

/// Ensure staging directory exists and return its path
pub fn ensure_staging_dir() -> Result<PathBuf> {
    let staging = staging_dir();
    fs::create_dir_all(&staging)?;
    Ok(staging)
}

/// Find the newest .html file in staging directory
pub fn find_staged_html() -> Option<PathBuf> {
    let staging = staging_dir();
    if !staging.exists() {
        return None;
    }

    fs::read_dir(&staging)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "html")
                .unwrap_or(false)
                || e.path()
                    .extension()
                    .map(|ext| ext == "htm")
                    .unwrap_or(false)
        })
        .max_by_key(|e| e.metadata().ok().and_then(|m| m.modified().ok()))
        .map(|e| e.path())
}

/// Clear any existing files in staging directory
pub fn clear_staging() -> Result<()> {
    let staging = staging_dir();
    if staging.exists() {
        for entry in fs::read_dir(&staging)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                fs::remove_file(&path)?;
            }
        }
    }
    Ok(())
}

// ============================================================================
// Step runners
// ============================================================================

/// Run the fetch_html step for a URL.
pub async fn run_fetch_html(url: &str, client: &CachingClient, force: bool) -> StepResult {
    let start = Instant::now();

    // Check if cached (unless force)
    let was_cached = if !force && client.is_cached(url) {
        // Already cached, and we have the content
        if client.get_cached_error(url).is_some() {
            // Cached error
            let duration_ms = start.elapsed().as_millis() as u64;
            let error = client.get_cached_error(url).unwrap_or_default();
            return StepResult {
                step: PipelineStep::FetchHtml,
                success: false,
                duration_ms,
                error: Some(format!("Cached error: {}", error)),
                cached: true,
            };
        }
        if client.get_cached_html(url).is_some() {
            // Cached success - we're done
            let duration_ms = start.elapsed().as_millis() as u64;
            return StepResult {
                step: PipelineStep::FetchHtml,
                success: true,
                duration_ms,
                error: None,
                cached: true,
            };
        }
        false
    } else {
        false
    };

    // Fetch (this will use cache internally with ETag validation if not force)
    let result = client.fetch_html(url).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(_) => StepResult {
            step: PipelineStep::FetchHtml,
            success: true,
            duration_ms,
            error: None,
            cached: was_cached,
        },
        Err(e) => StepResult {
            step: PipelineStep::FetchHtml,
            success: false,
            duration_ms,
            error: Some(e.to_string()),
            cached: false,
        },
    }
}

/// Run the extract_recipe step for a URL.
/// Requires HTML to be cached first.
pub fn run_extract_recipe(url: &str, client: &CachingClient, run_dir: &Path) -> ExtractStepResult {
    let start = Instant::now();
    let slug = slugify_url(url);
    let output_dir = run_dir.join("urls").join(&slug).join("extract_recipe");

    // Get HTML from cache
    let html = match client.get_cached_html(url) {
        Some(html) => html,
        None => {
            let duration_ms = start.elapsed().as_millis() as u64;
            return ExtractStepResult {
                step_result: StepResult {
                    step: PipelineStep::ExtractRecipe,
                    success: false,
                    duration_ms,
                    error: Some("HTML not cached - run fetch_html first".to_string()),
                    cached: false,
                },
                extraction_stats: None,
            };
        }
    };

    // Extract recipe with stats
    match ramekin_core::extract_recipe_with_stats(&html, url) {
        Ok(output) => {
            let duration_ms = start.elapsed().as_millis() as u64;

            // Build extraction stats from the attempts
            let stats = ExtractionStats {
                method_used: output.method_used,
                jsonld_success: output
                    .all_attempts
                    .iter()
                    .any(|a| a.method == ramekin_core::ExtractionMethod::JsonLd && a.success),
                microdata_success: output
                    .all_attempts
                    .iter()
                    .any(|a| a.method == ramekin_core::ExtractionMethod::Microdata && a.success),
            };

            // Save output
            if let Err(e) = save_extract_output(&output_dir, &output, duration_ms) {
                return ExtractStepResult {
                    step_result: StepResult {
                        step: PipelineStep::ExtractRecipe,
                        success: false,
                        duration_ms,
                        error: Some(format!("Failed to save output: {}", e)),
                        cached: false,
                    },
                    extraction_stats: Some(stats),
                };
            }

            ExtractStepResult {
                step_result: StepResult {
                    step: PipelineStep::ExtractRecipe,
                    success: true,
                    duration_ms,
                    error: None,
                    cached: false,
                },
                extraction_stats: Some(stats),
            }
        }
        Err(e) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            let error_msg = e.to_string();

            // Save error
            let _ = save_step_error(&output_dir, &error_msg, duration_ms);

            // Even on failure, we know neither method worked
            let stats = ExtractionStats {
                method_used: ramekin_core::ExtractionMethod::JsonLd, // Placeholder, not actually used
                jsonld_success: false,
                microdata_success: false,
            };

            ExtractStepResult {
                step_result: StepResult {
                    step: PipelineStep::ExtractRecipe,
                    success: false,
                    duration_ms,
                    error: Some(error_msg),
                    cached: false,
                },
                extraction_stats: Some(stats),
            }
        }
    }
}

/// Run the save_recipe step for a URL.
/// Requires extract_recipe to have run first.
pub fn run_save_recipe(url: &str, run_dir: &Path) -> StepResult {
    let start = Instant::now();
    let slug = slugify_url(url);
    let extract_dir = run_dir.join("urls").join(&slug).join("extract_recipe");
    let output_dir = run_dir.join("urls").join(&slug).join("save_recipe");

    // Load extract output
    let extract_output_path = extract_dir.join("output.json");
    let extract_output: ramekin_core::ExtractRecipeOutput =
        match fs::read_to_string(&extract_output_path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(output) => output,
                Err(e) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    return StepResult {
                        step: PipelineStep::SaveRecipe,
                        success: false,
                        duration_ms,
                        error: Some(format!("Failed to parse extract output: {}", e)),
                        cached: false,
                    };
                }
            },
            Err(_) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                return StepResult {
                    step: PipelineStep::SaveRecipe,
                    success: false,
                    duration_ms,
                    error: Some("Extract output not found - run extract_recipe first".to_string()),
                    cached: false,
                };
            }
        };

    let duration_ms = start.elapsed().as_millis() as u64;

    // Save the recipe to disk
    let save_output = ramekin_core::SaveRecipeOutput {
        raw_recipe: extract_output.raw_recipe,
        saved_at: Utc::now().to_rfc3339(),
    };

    if let Err(e) = save_save_output(&output_dir, &save_output, duration_ms) {
        return StepResult {
            step: PipelineStep::SaveRecipe,
            success: false,
            duration_ms,
            error: Some(format!("Failed to save output: {}", e)),
            cached: false,
        };
    }

    StepResult {
        step: PipelineStep::SaveRecipe,
        success: true,
        duration_ms,
        error: None,
        cached: false,
    }
}

/// Run the enrich_recipe step for a URL.
/// Requires extract_recipe to have run first.
/// Enrichment is optional - if provider is None, this step is skipped.
pub async fn run_enrich_recipe(
    url: &str,
    run_dir: &Path,
    provider: Option<&dyn LlmProvider>,
) -> EnrichStepResult {
    let start = Instant::now();
    let slug = slugify_url(url);
    let extract_dir = run_dir.join("urls").join(&slug).join("extract_recipe");
    let output_dir = run_dir.join("urls").join(&slug).join("enrich_recipe");

    // If no provider, skip enrichment
    let provider = match provider {
        Some(p) => p,
        None => {
            let duration_ms = start.elapsed().as_millis() as u64;
            return EnrichStepResult {
                step_result: StepResult {
                    step: PipelineStep::EnrichRecipe,
                    success: true,
                    duration_ms,
                    error: None,
                    cached: false,
                },
                enrichment_errors: vec![],
                skipped: true,
            };
        }
    };

    // Load extract output
    let extract_output_path = extract_dir.join("output.json");
    let extract_output: ramekin_core::ExtractRecipeOutput =
        match fs::read_to_string(&extract_output_path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(output) => output,
                Err(e) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    return EnrichStepResult {
                        step_result: StepResult {
                            step: PipelineStep::EnrichRecipe,
                            success: false,
                            duration_ms,
                            error: Some(format!("Failed to parse extract output: {}", e)),
                            cached: false,
                        },
                        enrichment_errors: vec![],
                        skipped: false,
                    };
                }
            },
            Err(_) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                return EnrichStepResult {
                    step_result: StepResult {
                        step: PipelineStep::EnrichRecipe,
                        success: false,
                        duration_ms,
                        error: Some(
                            "Extract output not found - run extract_recipe first".to_string(),
                        ),
                        cached: false,
                    },
                    enrichment_errors: vec![],
                    skipped: false,
                };
            }
        };

    // Convert RawRecipe to RecipeContent for enrichment
    let recipe_content = raw_recipe_to_content(&extract_output.raw_recipe);

    // Run all enrichments
    let (enriched, errors) = run_all_enrichments(provider, &recipe_content).await;

    let duration_ms = start.elapsed().as_millis() as u64;

    // Convert error types for serialization
    let enrichment_errors: Vec<(String, String)> = errors
        .into_iter()
        .map(|(name, e)| (name, e.to_string()))
        .collect();

    // Save the enriched output
    let enrich_output = EnrichRecipeOutput {
        original: recipe_content,
        enriched: enriched.clone(),
        enrichment_errors: enrichment_errors.clone(),
    };

    if let Err(e) = save_enrich_output(&output_dir, &enrich_output, duration_ms) {
        return EnrichStepResult {
            step_result: StepResult {
                step: PipelineStep::EnrichRecipe,
                success: false,
                duration_ms,
                error: Some(format!("Failed to save output: {}", e)),
                cached: false,
            },
            enrichment_errors,
            skipped: false,
        };
    }

    // Update the extract output with enriched data for downstream steps
    let enriched_raw = content_to_raw_recipe(&enriched, &extract_output.raw_recipe);
    let updated_extract = ramekin_core::ExtractRecipeOutput {
        raw_recipe: enriched_raw,
        method_used: extract_output.method_used,
        all_attempts: extract_output.all_attempts,
    };
    let _ = save_extract_output(&extract_dir, &updated_extract, 0);

    EnrichStepResult {
        step_result: StepResult {
            step: PipelineStep::EnrichRecipe,
            success: true,
            duration_ms,
            error: None,
            cached: false,
        },
        enrichment_errors,
        skipped: false,
    }
}

/// Output from the enrich_recipe step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichRecipeOutput {
    pub original: RecipeContent,
    pub enriched: RecipeContent,
    pub enrichment_errors: Vec<(String, String)>,
}

/// Result from the enrich step with enrichment-specific data.
#[derive(Debug, Clone)]
pub struct EnrichStepResult {
    pub step_result: StepResult,
    pub enrichment_errors: Vec<(String, String)>,
    pub skipped: bool,
}

/// Convert RawRecipe to RecipeContent for enrichment.
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
        description: raw.description.clone(),
        ingredients,
        instructions: raw.instructions.clone(),
        source_url: Some(raw.source_url.clone()),
        source_name: raw.source_name.clone(),
        tags: vec![],
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

/// Convert enriched RecipeContent back to RawRecipe format.
fn content_to_raw_recipe(content: &RecipeContent, original: &RawRecipe) -> RawRecipe {
    // Convert structured ingredients back to newline-separated string
    let ingredients = content
        .ingredients
        .iter()
        .map(|ing| {
            let mut parts = Vec::new();
            if let Some(ref amount) = ing.amount {
                parts.push(amount.clone());
            }
            if let Some(ref unit) = ing.unit {
                parts.push(unit.clone());
            }
            parts.push(ing.item.clone());
            if let Some(ref note) = ing.note {
                parts.push(format!("({})", note));
            }
            parts.join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n");

    RawRecipe {
        title: content.title.clone(),
        description: content.description.clone(),
        ingredients,
        instructions: content.instructions.clone(),
        image_urls: original.image_urls.clone(),
        source_url: content
            .source_url
            .clone()
            .unwrap_or_else(|| original.source_url.clone()),
        source_name: content.source_name.clone(),
    }
}

fn save_enrich_output(
    output_dir: &Path,
    output: &EnrichRecipeOutput,
    duration_ms: u64,
) -> Result<()> {
    fs::create_dir_all(output_dir)?;

    let json = serde_json::to_string_pretty(&output)?;
    fs::write(output_dir.join("output.json"), json)?;

    let timing = StepTiming {
        started_at: Utc::now().to_rfc3339(),
        completed_at: Utc::now().to_rfc3339(),
        duration_ms,
    };
    let timing_json = serde_json::to_string_pretty(&timing)?;
    fs::write(output_dir.join("timing.json"), timing_json)?;

    // Remove any existing error file
    let error_path = output_dir.join("error.txt");
    if error_path.exists() {
        let _ = fs::remove_file(error_path);
    }

    Ok(())
}

// ============================================================================
// Output saving helpers
// ============================================================================

fn save_extract_output(
    output_dir: &Path,
    output: &ramekin_core::ExtractRecipeOutput,
    duration_ms: u64,
) -> Result<()> {
    fs::create_dir_all(output_dir)?;

    let json = serde_json::to_string_pretty(&output)?;
    fs::write(output_dir.join("output.json"), json)?;

    let timing = StepTiming {
        started_at: Utc::now().to_rfc3339(), // Approximate
        completed_at: Utc::now().to_rfc3339(),
        duration_ms,
    };
    let timing_json = serde_json::to_string_pretty(&timing)?;
    fs::write(output_dir.join("timing.json"), timing_json)?;

    // Remove any existing error file
    let error_path = output_dir.join("error.txt");
    if error_path.exists() {
        let _ = fs::remove_file(error_path);
    }

    Ok(())
}

fn save_save_output(
    output_dir: &Path,
    save_output: &ramekin_core::SaveRecipeOutput,
    duration_ms: u64,
) -> Result<()> {
    fs::create_dir_all(output_dir)?;

    let json = serde_json::to_string_pretty(&save_output)?;
    fs::write(output_dir.join("output.json"), json)?;

    let timing = StepTiming {
        started_at: Utc::now().to_rfc3339(), // Approximate
        completed_at: Utc::now().to_rfc3339(),
        duration_ms,
    };
    let timing_json = serde_json::to_string_pretty(&timing)?;
    fs::write(output_dir.join("timing.json"), timing_json)?;

    // Remove any existing error file
    let error_path = output_dir.join("error.txt");
    if error_path.exists() {
        let _ = fs::remove_file(error_path);
    }

    Ok(())
}

fn save_step_error(output_dir: &Path, error: &str, duration_ms: u64) -> Result<()> {
    fs::create_dir_all(output_dir)?;

    fs::write(output_dir.join("error.txt"), error)?;

    let timing = StepTiming {
        started_at: Utc::now().to_rfc3339(),
        completed_at: Utc::now().to_rfc3339(),
        duration_ms,
    };
    let timing_json = serde_json::to_string_pretty(&timing)?;
    fs::write(output_dir.join("timing.json"), timing_json)?;

    Ok(())
}

// ============================================================================
// Run all steps for a URL
// ============================================================================

/// Run all pipeline steps for a URL.
///
/// If `llm_provider` is Some, enrichment will be run after extraction.
/// If None, the enrich step is skipped.
pub async fn run_all_steps(
    url: &str,
    client: &CachingClient,
    run_dir: &Path,
    force_fetch: bool,
    llm_provider: Option<&dyn LlmProvider>,
) -> AllStepsResult {
    let mut step_results = Vec::new();
    let mut extraction_stats = None;
    let mut enrichment_errors = Vec::new();

    // Step 1: Fetch HTML
    let fetch_result = run_fetch_html(url, client, force_fetch).await;
    let fetch_success = fetch_result.success;
    step_results.push(fetch_result);

    if !fetch_success {
        return AllStepsResult {
            step_results,
            extraction_stats,
            enrichment_errors,
        };
    }

    // Step 2: Extract Recipe
    let extract_result = run_extract_recipe(url, client, run_dir);
    extraction_stats = extract_result.extraction_stats;
    let extract_success = extract_result.step_result.success;
    step_results.push(extract_result.step_result);

    if !extract_success {
        return AllStepsResult {
            step_results,
            extraction_stats,
            enrichment_errors,
        };
    }

    // Step 3: Enrich Recipe (optional)
    let enrich_result = run_enrich_recipe(url, run_dir, llm_provider).await;
    enrichment_errors = enrich_result.enrichment_errors;
    let enrich_success = enrich_result.step_result.success;
    if !enrich_result.skipped {
        step_results.push(enrich_result.step_result);
    }

    if !enrich_success {
        return AllStepsResult {
            step_results,
            extraction_stats,
            enrichment_errors,
        };
    }

    // Step 4: Save Recipe
    let save_result = run_save_recipe(url, run_dir);
    step_results.push(save_result);

    AllStepsResult {
        step_results,
        extraction_stats,
        enrichment_errors,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_url() {
        assert_eq!(
            slugify_url("https://www.seriouseats.com/best-chili-recipe"),
            "seriouseats-com_best-chili-recipe"
        );

        assert_eq!(
            slugify_url("https://thekitchn.com/recipe/soup"),
            "thekitchn-com_recipe-soup"
        );

        assert_eq!(slugify_url("https://example.com/"), "example-com");
    }

    #[test]
    fn test_pipeline_step_from_str() {
        assert_eq!(
            PipelineStep::from_str("fetch_html").unwrap(),
            PipelineStep::FetchHtml
        );
        assert_eq!(
            PipelineStep::from_str("extract_recipe").unwrap(),
            PipelineStep::ExtractRecipe
        );
        assert_eq!(
            PipelineStep::from_str("enrich_recipe").unwrap(),
            PipelineStep::EnrichRecipe
        );
        assert_eq!(
            PipelineStep::from_str("save_recipe").unwrap(),
            PipelineStep::SaveRecipe
        );
        assert!(PipelineStep::from_str("invalid").is_err());
    }
}
