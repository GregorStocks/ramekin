//! CLI step runner functions for the pipeline orchestrator.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use ramekin_core::http::{slugify_url, CachingClient, HttpClient};
use ramekin_core::pipeline::{run_pipeline, StepOutputStore};
pub use ramekin_core::PipelineStep;

use super::output_store::FileOutputStore;

// ============================================================================
// Types
// ============================================================================

/// Result from a single pipeline step.
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

/// Stats about which extraction methods succeeded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionStats {
    pub method_used: ramekin_core::ExtractionMethod,
    pub jsonld_success: bool,
    pub microdata_success: bool,
}

/// Result from running all steps, including extraction stats.
#[derive(Debug, Clone)]
pub struct AllStepsResult {
    pub step_results: Vec<StepResult>,
    pub extraction_stats: Option<ExtractionStats>,
}

// ============================================================================
// Step parsing
// ============================================================================

/// Parse a pipeline step from string, returning an error for invalid steps.
pub fn parse_pipeline_step(s: &str) -> Result<PipelineStep> {
    PipelineStep::from_str(s).ok_or_else(|| {
        anyhow!(
            "Unknown step: {}. Valid steps: fetch_html, extract_recipe, save_recipe, enrich",
            s
        )
    })
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
pub fn run_extract_recipe(url: &str, client: &CachingClient, run_dir: &Path) -> StepResult {
    let start = Instant::now();
    let slug = slugify_url(url);
    let output_dir = run_dir.join("urls").join(&slug).join("extract_recipe");

    // Get HTML from cache
    let html = match client.get_cached_html(url) {
        Some(html) => html,
        None => {
            let duration_ms = start.elapsed().as_millis() as u64;
            return StepResult {
                step: PipelineStep::ExtractRecipe,
                success: false,
                duration_ms,
                error: Some("HTML not cached - run fetch_html first".to_string()),
                cached: false,
            };
        }
    };

    // Extract recipe with stats
    match ramekin_core::extract_recipe_with_stats(&html, url) {
        Ok(output) => {
            let duration_ms = start.elapsed().as_millis() as u64;

            // Save output
            if let Err(e) = save_extract_output(&output_dir, &output, duration_ms) {
                return StepResult {
                    step: PipelineStep::ExtractRecipe,
                    success: false,
                    duration_ms,
                    error: Some(format!("Failed to save output: {}", e)),
                    cached: false,
                };
            }

            StepResult {
                step: PipelineStep::ExtractRecipe,
                success: true,
                duration_ms,
                error: None,
                cached: false,
            }
        }
        Err(e) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            let error_msg = e.to_string();

            // Save error
            let _ = save_step_error(&output_dir, &error_msg, duration_ms);

            StepResult {
                step: PipelineStep::ExtractRecipe,
                success: false,
                duration_ms,
                error: Some(error_msg),
                cached: false,
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

/// Run the enrich step for a URL.
/// Currently a no-op that always fails - enrichment is expected to be unreliable.
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
// Run all steps
// ============================================================================

/// Run all pipeline steps for a URL using the generic pipeline infrastructure.
///
/// Takes an `Arc<CachingClient>` for shared ownership across pipeline steps.
pub async fn run_all_steps(
    url: &str,
    client: Arc<CachingClient>,
    run_dir: &Path,
    force_fetch: bool,
) -> AllStepsResult {
    use super::build_registry;

    let mut step_results = Vec::new();
    let mut store = FileOutputStore::new(run_dir, url);

    // Check if content is already cached (fast path to avoid network requests)
    let already_cached = !force_fetch
        && client.is_cached(url)
        && client.get_cached_html(url).is_some()
        && client.get_cached_error(url).is_none();

    // Determine starting point for the pipeline
    let first_step = if force_fetch {
        // Force fetch by running the dedicated fetch function
        let fetch_result = run_fetch_html(url, &client, true).await;
        step_results.push(fetch_result.clone());
        if !fetch_result.success {
            return AllStepsResult {
                step_results,
                extraction_stats: None,
            };
        }
        // After force fetch, pre-populate store and start from extract_recipe
        if let Some(html) = client.get_cached_html(url) {
            let _ = store.save_output("fetch_html", &serde_json::json!({ "html": html }));
        }
        "extract_recipe"
    } else if already_cached {
        // Content is cached - add a synthetic fetch result and skip to extract_recipe
        step_results.push(StepResult {
            step: PipelineStep::FetchHtml,
            success: true,
            duration_ms: 0,
            error: None,
            cached: true,
        });
        // Pre-populate store with cached HTML so extract_recipe can find it
        if let Some(html) = client.get_cached_html(url) {
            let _ = store.save_output("fetch_html", &serde_json::json!({ "html": html }));
        }
        "extract_recipe"
    } else {
        // Not cached - start from fetch_html
        "fetch_html"
    };

    // Build the registry with the shared client
    let registry = build_registry(client);

    // Run the generic pipeline from the determined starting point
    let generic_results = run_pipeline(first_step, url, &mut store, &registry).await;

    // Convert generic results to our StepResult format and append to any existing results
    let mut extraction_stats = None;

    for result in &generic_results {
        // Determine which step this is by looking at the output structure
        let step = if result.output.get("html").is_some() {
            PipelineStep::FetchHtml
        } else if result.output.get("method_used").is_some() {
            // This is extract_recipe - also extract the stats
            extraction_stats = extract_stats_from_output(&result.output);
            PipelineStep::ExtractRecipe
        } else if result.output.get("images_fetched").is_some() {
            // fetch_images - skip in our results since we didn't have it before
            continue;
        } else if result.output.get("saved_at").is_some() {
            PipelineStep::SaveRecipe
        } else {
            PipelineStep::Enrich
        };

        step_results.push(StepResult {
            step,
            success: result.success,
            duration_ms: result.duration_ms,
            error: result.error.clone(),
            cached: false, // Generic pipeline doesn't track this
        });
    }

    AllStepsResult {
        step_results,
        extraction_stats,
    }
}

/// Extract ExtractionStats from the extract_recipe output JSON.
fn extract_stats_from_output(output: &serde_json::Value) -> Option<ExtractionStats> {
    let method_used = output.get("method_used")?.as_str()?;
    let all_attempts = output.get("all_attempts")?.as_array()?;

    let method = match method_used {
        "jsonld" => ramekin_core::ExtractionMethod::JsonLd,
        "microdata" => ramekin_core::ExtractionMethod::Microdata,
        _ => return None,
    };

    let jsonld_success = all_attempts.iter().any(|a| {
        a.get("method").and_then(|m| m.as_str()) == Some("jsonld")
            && a.get("success").and_then(|s| s.as_bool()) == Some(true)
    });

    let microdata_success = all_attempts.iter().any(|a| {
        a.get("method").and_then(|m| m.as_str()) == Some("microdata")
            && a.get("success").and_then(|s| s.as_bool()) == Some(true)
    });

    Some(ExtractionStats {
        method_used: method,
        jsonld_success,
        microdata_success,
    })
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

fn save_save_output(
    output_dir: &Path,
    save_output: &ramekin_core::SaveRecipeOutput,
    duration_ms: u64,
) -> Result<()> {
    fs::create_dir_all(output_dir)?;

    let json = serde_json::to_string_pretty(&save_output)?;
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
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use ramekin_core::http::slugify_url;

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
