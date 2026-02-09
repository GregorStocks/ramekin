//! CLI step runner functions for the pipeline orchestrator.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use ramekin_core::http::{CachingClient, HttpClient};
use ramekin_core::pipeline::{run_pipeline, StepRegistry};
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

/// Stats about which extraction methods succeeded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionStats {
    pub method_used: ramekin_core::ExtractionMethod,
    pub jsonld_success: bool,
    pub microdata_success: bool,
}

/// Stats about ingredient parsing (volume-to-weight and metric conversions).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IngredientStats {
    pub total_ingredients: usize,
    pub volume_converted: usize,
    pub volume_unknown_ingredient: usize,
    pub volume_no_volume: usize,
    pub volume_already_has_weight: usize,
    pub metric_converted_oz: usize,
    pub metric_converted_lb: usize,
    /// Names of ingredients that had volume measurements but no density data.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unknown_ingredients: Vec<String>,
}

/// Result from running all steps, including extraction stats.
#[derive(Debug, Clone)]
pub struct AllStepsResult {
    pub step_results: Vec<StepResult>,
    pub extraction_stats: Option<ExtractionStats>,
    pub ingredient_stats: Option<IngredientStats>,
    /// Whether the AI auto-tag response was cached (None if auto-tag didn't run)
    pub ai_cached: Option<bool>,
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

// ============================================================================
// Run all steps
// ============================================================================

/// Run all pipeline steps for a URL using the generic pipeline infrastructure.
///
/// Takes an `Arc<CachingClient>` for shared ownership across pipeline steps,
/// plus a shared `StepRegistry` (built once per pipeline run).
pub async fn run_all_steps(
    url: &str,
    client: Arc<CachingClient>,
    run_dir: &Path,
    force_fetch: bool,
    registry: Arc<StepRegistry>,
) -> AllStepsResult {
    let _span = tracing::info_span!("run_all_steps").entered();

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
                ingredient_stats: None,
                ai_cached: None,
            };
        }
        // After force fetch, pre-populate store and start from extract_recipe
        // Only cache in memory - skip disk write since HTML is already in disk cache
        {
            let _span = tracing::info_span!("load_cached_html").entered();
            if let Some(html) = client.get_cached_html(url) {
                store.cache_only("fetch_html", serde_json::json!({ "html": html }));
            }
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
        // Only cache in memory - skip disk write since HTML is already in disk cache
        {
            let _span = tracing::info_span!("load_cached_html").entered();
            if let Some(html) = client.get_cached_html(url) {
                store.cache_only("fetch_html", serde_json::json!({ "html": html }));
            }
        }
        "extract_recipe"
    } else {
        // Not cached - start from fetch_html
        "fetch_html"
    };

    // Run the generic pipeline from the determined starting point
    let generic_results = run_pipeline(first_step, url, &mut store, &registry).await;

    // Convert generic results to our StepResult format and append to any existing results
    let mut extraction_stats = None;
    let mut ingredient_stats = None;
    let mut ai_cached = None;

    for result in &generic_results {
        // Use step_name for reliable step identification
        let step = match PipelineStep::from_str(&result.step_name) {
            Some(s) => s,
            None => continue, // Skip unknown steps
        };

        // Extract stats for extract_recipe step
        if step == PipelineStep::ExtractRecipe {
            extraction_stats = extract_stats_from_output(&result.output);
        }

        // Extract stats for parse_ingredients step
        if step == PipelineStep::ParseIngredients && result.success {
            ingredient_stats = extract_ingredient_stats_from_output(&result.output);
        }

        // Extract AI cache status from auto-tag step
        if step == PipelineStep::EnrichAutoTag && result.success {
            ai_cached = result.output.get("cached").and_then(|v| v.as_bool());
        }

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
        ingredient_stats,
        ai_cached,
    }
}

/// Extract ExtractionStats from the extract_recipe output JSON.
fn extract_stats_from_output(output: &serde_json::Value) -> Option<ExtractionStats> {
    let method_used = output.get("method_used")?.as_str()?;
    let all_attempts = output.get("all_attempts")?.as_array()?;

    let method = match method_used {
        "json_ld" => ramekin_core::ExtractionMethod::JsonLd,
        "microdata" => ramekin_core::ExtractionMethod::Microdata,
        "html_fallback" => ramekin_core::ExtractionMethod::HtmlFallback,
        _ => return None,
    };

    let jsonld_success = all_attempts.iter().any(|a| {
        a.get("method").and_then(|m| m.as_str()) == Some("json_ld")
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

/// Extract IngredientStats from the parse_ingredients output JSON.
fn extract_ingredient_stats_from_output(output: &serde_json::Value) -> Option<IngredientStats> {
    let ingredients = output.get("ingredients")?.as_array()?;
    let total_ingredients = ingredients.len();

    let volume_stats = output.get("volume_stats")?;
    let metric_stats = output.get("metric_stats")?;

    Some(IngredientStats {
        total_ingredients,
        volume_converted: volume_stats
            .get("converted")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize,
        volume_unknown_ingredient: volume_stats
            .get("skipped_unknown_ingredient")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize,
        volume_no_volume: volume_stats
            .get("skipped_no_volume")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize,
        volume_already_has_weight: volume_stats
            .get("skipped_already_has_weight")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize,
        metric_converted_oz: metric_stats
            .get("converted_oz")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize,
        metric_converted_lb: metric_stats
            .get("converted_lb")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize,
        unknown_ingredients: volume_stats
            .get("unknown_ingredients")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
    })
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
        assert_eq!(
            PipelineStep::from_str("enrich_normalize_ingredients"),
            Some(PipelineStep::EnrichNormalizeIngredients)
        );
        assert_eq!(
            PipelineStep::from_str("enrich_auto_tag"),
            Some(PipelineStep::EnrichAutoTag)
        );
        assert_eq!(
            PipelineStep::from_str("enrich_generate_photo"),
            Some(PipelineStep::EnrichGeneratePhoto)
        );
        assert_eq!(PipelineStep::from_str("invalid"), None);
    }
}
