use anyhow::{anyhow, Result};
use chrono::Utc;
use ramekin_core::http::{CachingClient, HttpClient};
// Re-export PipelineStep from ramekin_core for backwards compatibility
pub use ramekin_core::PipelineStep;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Parse a pipeline step from string, returning an error for invalid steps.
/// This wraps PipelineStep::from_str with proper error handling for CLI usage.
pub fn parse_pipeline_step(s: &str) -> Result<PipelineStep> {
    PipelineStep::from_str(s).ok_or_else(|| {
        anyhow!(
            "Unknown step: {}. Valid steps: fetch_html, extract_recipe, save_recipe, enrich",
            s
        )
    })
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

/// Result from running all steps, including extraction stats
#[derive(Debug, Clone)]
pub struct AllStepsResult {
    pub step_results: Vec<StepResult>,
    pub extraction_stats: Option<ExtractionStats>,
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
// Enrich step runner
// ============================================================================

/// Run the enrich step for a URL.
/// Currently a no-op that always fails - enrichment is expected to be unreliable.
/// The pipeline will continue regardless (enrichment failures are non-fatal).
pub fn run_enrich(url: &str, run_dir: &Path) -> StepResult {
    let start = Instant::now();
    let slug = slugify_url(url);
    let output_dir = run_dir.join("urls").join(&slug).join("enrich");
    let duration_ms = start.elapsed().as_millis() as u64;

    let error_msg = "Enrichment not implemented (no-op stub)".to_string();
    let _ = save_step_error(&output_dir, &error_msg, duration_ms);

    StepResult {
        step: PipelineStep::Enrich,
        success: false,
        duration_ms,
        error: Some(error_msg),
        cached: false,
    }
}

// ============================================================================
// Run all steps for a URL
// ============================================================================

/// Run all pipeline steps for a URL.
pub async fn run_all_steps(
    url: &str,
    client: &CachingClient,
    run_dir: &Path,
    force_fetch: bool,
) -> AllStepsResult {
    let mut step_results = Vec::new();
    let mut extraction_stats = None;

    // Step 1: Fetch HTML
    let fetch_result = run_fetch_html(url, client, force_fetch).await;
    let fetch_success = fetch_result.success;
    step_results.push(fetch_result);

    if !fetch_success {
        return AllStepsResult {
            step_results,
            extraction_stats,
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
        };
    }

    // Step 3: Save Recipe
    let save_result = run_save_recipe(url, run_dir);
    let save_success = save_result.success;
    step_results.push(save_result);

    if !save_success {
        return AllStepsResult {
            step_results,
            extraction_stats,
        };
    }

    // Step 4: Enrich (always runs, continues on failure)
    // Note: We skip FetchImages as it's DB-specific
    let enrich_result = run_enrich(url, run_dir);
    step_results.push(enrich_result);

    AllStepsResult {
        step_results,
        extraction_stats,
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
            PipelineStep::from_str("fetch_html"),
            Some(PipelineStep::FetchHtml)
        );
        assert_eq!(
            PipelineStep::from_str("extract_recipe"),
            Some(PipelineStep::ExtractRecipe)
        );
        assert_eq!(
            PipelineStep::from_str("save_recipe"),
            Some(PipelineStep::SaveRecipe)
        );
        assert_eq!(PipelineStep::from_str("enrich"), Some(PipelineStep::Enrich));
        assert_eq!(PipelineStep::from_str("invalid"), None);
    }

    #[test]
    fn test_parse_pipeline_step() {
        assert!(parse_pipeline_step("fetch_html").is_ok());
        assert!(parse_pipeline_step("extract_recipe").is_ok());
        assert!(parse_pipeline_step("save_recipe").is_ok());
        assert!(parse_pipeline_step("enrich").is_ok());
        assert!(parse_pipeline_step("invalid").is_err());
    }
}
