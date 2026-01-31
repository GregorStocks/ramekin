use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Utc;
use futures::stream::{self, StreamExt};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::generate_test_urls::TestUrlsOutput;
use crate::pipeline::{
    clear_staging, ensure_staging_dir, find_staged_html, run_all_steps, staging_dir,
    AllStepsResult, ExtractionStats, IngredientStats, PipelineStep, StepResult,
};
use crate::OnFetchFail;
use ramekin_core::http::{CachingClient, DiskCache};

// ============================================================================
// Configuration
// ============================================================================

pub struct OrchestratorConfig {
    pub test_urls_file: PathBuf,
    pub output_dir: PathBuf,
    pub limit: Option<usize>,
    pub site_filter: Option<String>,
    pub delay_ms: u64,
    pub offline: bool,
    pub force_refetch: bool,
    pub on_fetch_fail: OnFetchFail,
    pub tags_file: PathBuf,
    pub concurrency: usize,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            test_urls_file: PathBuf::from("data/test-urls.json"),
            output_dir: PathBuf::from("data/pipeline-runs"),
            limit: None,
            site_filter: None,
            delay_ms: 1000,
            offline: true,
            force_refetch: false,
            on_fetch_fail: OnFetchFail::Continue,
            tags_file: PathBuf::from("data/eval-tags.json"),
            concurrency: 10,
        }
    }
}

/// Tags file format for evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagsFile {
    pub tags: Vec<String>,
}

/// Load tags from a JSON file.
pub fn load_tags_file(path: &Path) -> Result<Vec<String>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read tags file: {}", path.display()))?;
    let tags_file: TagsFile =
        serde_json::from_str(&content).with_context(|| "Failed to parse tags file as JSON")?;
    Ok(tags_file.tags)
}

// ============================================================================
// Run manifest
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunManifest {
    pub run_id: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub config: ManifestConfig,
    pub status: RunStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestConfig {
    pub test_urls_file: String,
    pub limit: Option<usize>,
    pub site_filter: Option<String>,
    pub delay_ms: u64,
    pub offline: bool,
    pub force_refetch: bool,
    pub concurrency: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
}

// ============================================================================
// Results
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PipelineResults {
    pub total_urls: usize,
    pub completed: usize,
    pub failed_at_fetch: usize,
    pub failed_at_extract: usize,
    pub failed_at_save: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub ai_cache_hits: usize,
    pub ai_cache_misses: usize,
    pub by_site: HashMap<String, SiteResults>,
    pub url_results: Vec<UrlResult>,
    pub extraction_method_stats: ExtractionMethodStats,
    pub ingredient_stats: IngredientParsingStats,
}

/// Aggregated stats about ingredient parsing across all URLs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IngredientParsingStats {
    /// Total ingredients parsed across all recipes
    pub total_ingredients: usize,
    /// Volume-to-weight conversions successful
    pub volume_converted: usize,
    /// Volume-to-weight conversions failed (unknown ingredient)
    pub volume_unknown_ingredient: usize,
    /// Volume-to-weight conversions skipped (no volume unit)
    pub volume_no_volume: usize,
    /// Volume-to-weight conversions skipped (already has weight)
    pub volume_already_has_weight: usize,
    /// Metric conversions from oz
    pub metric_converted_oz: usize,
    /// Metric conversions from lb
    pub metric_converted_lb: usize,
}

/// Stats about which extraction methods work across all URLs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractionMethodStats {
    /// URLs where HTML was fetched successfully (denominator for extraction stats)
    pub urls_with_html: usize,
    /// URLs where JSON-LD extraction succeeded
    pub jsonld_success: usize,
    /// URLs where microdata extraction succeeded
    pub microdata_success: usize,
    /// URLs where both methods succeeded
    pub both_success: usize,
    /// URLs where neither method succeeded
    pub neither_success: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteResults {
    pub domain: String,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlResult {
    pub url: String,
    pub site: String,
    pub steps: Vec<StepResult>,
    pub final_status: FinalStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extraction_stats: Option<ExtractionStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalStatus {
    Completed,
    FailedAtFetch,
    FailedAtExtract,
    FailedAtSave,
}

// ============================================================================
// Main orchestrator
// ============================================================================

pub async fn run_pipeline_test(config: OrchestratorConfig) -> Result<PipelineResults> {
    // Generate run ID
    let now = Utc::now();
    let run_id = now.format("%Y-%m-%d_%H-%M-%S").to_string();
    let run_dir = config.output_dir.join(&run_id);

    // Create run directory
    fs::create_dir_all(&run_dir)?;

    // Load test URLs
    let test_urls_content = fs::read_to_string(&config.test_urls_file).with_context(|| {
        format!(
            "Failed to read test URLs from {}",
            config.test_urls_file.display()
        )
    })?;
    let test_urls: TestUrlsOutput =
        serde_json::from_str(&test_urls_content).context("Failed to parse test URLs JSON")?;

    // Collect URLs to process
    let mut urls_to_process: Vec<(String, String)> = Vec::new(); // (url, domain)

    for site in &test_urls.sites {
        // Apply site filter
        if let Some(ref filter) = config.site_filter {
            if !site.domain.contains(filter) {
                continue;
            }
        }

        for url in &site.urls {
            urls_to_process.push((url.clone(), site.domain.clone()));
        }
    }

    // Apply limit
    if let Some(limit) = config.limit {
        urls_to_process.truncate(limit);
    }

    // Load tags for auto-tag evaluation
    let user_tags = load_tags_file(&config.tags_file)?;
    println!(
        "Loaded {} tags from {}",
        user_tags.len(),
        config.tags_file.display()
    );

    // Create manifest
    let manifest = RunManifest {
        run_id: run_id.clone(),
        started_at: now.to_rfc3339(),
        completed_at: None,
        config: ManifestConfig {
            test_urls_file: config.test_urls_file.display().to_string(),
            limit: config.limit,
            site_filter: config.site_filter.clone(),
            delay_ms: config.delay_ms,
            offline: config.offline,
            force_refetch: config.force_refetch,
            concurrency: config.concurrency,
        },
        status: RunStatus::Running,
    };
    save_manifest(&run_dir, &manifest)?;

    // Initialize HTTP client with caching
    // The CachingClient uses RAMEKIN_HTTP_CACHE env var for cache directory
    // and handles rate limiting internally
    // Use never_network mode when offline - this means:
    // - Cached responses are used directly without network validation
    // - Uncached URLs will error instead of fetching (use --offline=false to enable network)
    let client = Arc::new(
        CachingClient::builder()
            .rate_limit_ms(0) // We handle delay ourselves between URLs
            .never_network(config.offline)
            .build()
            .context("Failed to create HTTP client")?,
    );

    let total_urls = urls_to_process.len();
    let start_time = Instant::now();

    println!("Pipeline Test Starting");
    println!("======================");
    println!("Run ID: {}", run_id);
    println!("URLs to process: {}", total_urls);
    if let Some(ref filter) = config.site_filter {
        println!("Site filter: {}", filter);
    }
    println!();

    // In prompt mode, ensure staging directory exists and is empty
    // Also force concurrency=1 since interactive prompts don't work well with parallelism
    let concurrency = if matches!(config.on_fetch_fail, OnFetchFail::Prompt) {
        ensure_staging_dir()?;
        clear_staging()?;
        println!(
            "Interactive mode: save HTML files to {}",
            staging_dir().display()
        );
        println!("(concurrency forced to 1 for interactive mode)");
        println!();
        1
    } else {
        println!("Concurrency: {}", config.concurrency);
        println!();
        config.concurrency
    };

    // Shuffle URLs to interleave domains for better parallelism
    // (avoids all concurrent slots hitting the same domain)
    urls_to_process.shuffle(&mut rand::rng());

    // Shared state for concurrent processing
    let results = Arc::new(Mutex::new(PipelineResults {
        total_urls,
        ..Default::default()
    }));
    let completed_count = Arc::new(AtomicUsize::new(0));
    let run_dir = Arc::new(run_dir);

    // Process URLs concurrently
    let url_results: Vec<Option<UrlResult>> = stream::iter(urls_to_process.into_iter())
        .map(|(url, domain)| {
            let client = Arc::clone(&client);
            let run_dir = Arc::clone(&run_dir);
            let user_tags = user_tags.clone();
            let results = Arc::clone(&results);
            let completed_count = Arc::clone(&completed_count);
            let on_fetch_fail = config.on_fetch_fail;
            let force_refetch = config.force_refetch;
            let offline = config.offline;

            async move {
                // Check if we need to fetch (for progress display)
                let needs_fetch = force_refetch || (!offline && !client.is_cached(&url));

                // Increment and get progress
                let completed = completed_count.fetch_add(1, Ordering::SeqCst) + 1;
                let progress = format!("[{}/{}]", completed, total_urls);

                // Print progress
                if force_refetch {
                    println!("{} {} (force refetch)", progress, truncate_url(&url, 60));
                } else if needs_fetch {
                    println!("{} {} (fetching...)", progress, truncate_url(&url, 60));
                } else {
                    println!("{} {} (cached)", progress, truncate_url(&url, 60));
                }

                // Run all steps
                let mut all_results = run_all_steps(
                    &url,
                    Arc::clone(&client),
                    &run_dir,
                    force_refetch,
                    user_tags.clone(),
                )
                .await;

                // Check if fetch failed
                let fetch_failed = all_results
                    .step_results
                    .first()
                    .map(|r| r.step == PipelineStep::FetchHtml && !r.success)
                    .unwrap_or(false);

                if fetch_failed {
                    match on_fetch_fail {
                        OnFetchFail::Skip => {
                            println!("  -> Skipped (fetch failed)");
                            return None;
                        }
                        OnFetchFail::Prompt => {
                            // Interactive mode: prompt user to save HTML
                            if let Ok(Some(new_results)) = prompt_for_manual_cache(
                                &url,
                                Arc::clone(&client),
                                &run_dir,
                                user_tags,
                            )
                            .await
                            {
                                all_results = new_results;
                            }
                            // If user skipped, all_results still has the failed fetch
                        }
                        OnFetchFail::Continue => {
                            // Default: just continue (already have failed result)
                        }
                    }
                }

                // Determine final status
                let final_status = determine_final_status(&all_results.step_results);

                // Update shared results
                {
                    let mut results = results.lock().await;
                    update_results(
                        &mut results,
                        &all_results.step_results,
                        &final_status,
                        &domain,
                        all_results.extraction_stats.as_ref(),
                        all_results.ingredient_stats.as_ref(),
                        all_results.ai_cached,
                    );

                    // Save intermediate results periodically
                    if let Err(e) = save_results(&run_dir, &results) {
                        tracing::warn!(error = %e, "Failed to save intermediate results");
                    }
                }

                Some(UrlResult {
                    url,
                    site: domain,
                    steps: all_results.step_results,
                    final_status,
                    extraction_stats: all_results.extraction_stats,
                })
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    // Collect all URL results into the final results
    {
        let mut results = results.lock().await;
        for url_result in url_results.into_iter().flatten() {
            results.url_results.push(url_result);
        }
    }

    // Extract final results from Arc<Mutex<>>
    let results = Arc::try_unwrap(results)
        .expect("All references should be dropped")
        .into_inner();

    let elapsed = start_time.elapsed();

    // Update manifest with completion
    let final_manifest = RunManifest {
        completed_at: Some(Utc::now().to_rfc3339()),
        status: RunStatus::Completed,
        ..manifest
    };
    save_manifest(&run_dir, &final_manifest)?;

    // Print summary
    println!();
    println!("Pipeline Test Results");
    println!("=====================");
    println!("Run ID: {}", run_id);
    println!("Duration: {:.1}s", elapsed.as_secs_f64());
    println!("URLs Processed: {}", results.total_urls);
    println!();
    println!("Cache Stats:");
    println!(
        "  HTML cache hits: {} ({:.1}%)",
        results.cache_hits,
        if results.total_urls > 0 {
            results.cache_hits as f64 / results.total_urls as f64 * 100.0
        } else {
            0.0
        }
    );
    println!("  HTML cache misses: {} (fetched)", results.cache_misses);
    let ai_total = results.ai_cache_hits + results.ai_cache_misses;
    if ai_total > 0 {
        println!(
            "  AI cache hits: {} ({:.1}%)",
            results.ai_cache_hits,
            results.ai_cache_hits as f64 / ai_total as f64 * 100.0
        );
        println!("  AI cache misses: {} (API calls)", results.ai_cache_misses);
    }
    println!();
    println!("Overall Results:");
    println!(
        "  Completed: {} ({:.1}%)",
        results.completed,
        if results.total_urls > 0 {
            results.completed as f64 / results.total_urls as f64 * 100.0
        } else {
            0.0
        }
    );
    println!(
        "  Failed at fetch_html: {} ({:.1}%)",
        results.failed_at_fetch,
        if results.total_urls > 0 {
            results.failed_at_fetch as f64 / results.total_urls as f64 * 100.0
        } else {
            0.0
        }
    );
    println!(
        "  Failed at extract_recipe: {} ({:.1}%)",
        results.failed_at_extract,
        if results.total_urls > 0 {
            results.failed_at_extract as f64 / results.total_urls as f64 * 100.0
        } else {
            0.0
        }
    );
    println!(
        "  Failed at save_recipe: {} ({:.1}%)",
        results.failed_at_save,
        if results.total_urls > 0 {
            results.failed_at_save as f64 / results.total_urls as f64 * 100.0
        } else {
            0.0
        }
    );
    println!();
    println!("Results by Site:");

    // Sort sites by completion rate
    let mut sites: Vec<_> = results.by_site.values().collect();
    sites.sort_by(|a, b| {
        let rate_a = if a.total > 0 {
            a.completed as f64 / a.total as f64
        } else {
            0.0
        };
        let rate_b = if b.total > 0 {
            b.completed as f64 / b.total as f64
        } else {
            0.0
        };
        rate_b
            .partial_cmp(&rate_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for site in &sites {
        let rate = if site.total > 0 {
            site.completed as f64 / site.total as f64 * 100.0
        } else {
            0.0
        };
        println!(
            "  {}: {}/{} ({:.1}%)",
            site.domain, site.completed, site.total, rate
        );
    }

    println!();
    println!("Extraction Method Stats:");
    let ems = &results.extraction_method_stats;
    if ems.urls_with_html > 0 {
        println!(
            "  JSON-LD: {}/{} ({:.1}%)",
            ems.jsonld_success,
            ems.urls_with_html,
            ems.jsonld_success as f64 / ems.urls_with_html as f64 * 100.0
        );
        println!(
            "  Microdata: {}/{} ({:.1}%)",
            ems.microdata_success,
            ems.urls_with_html,
            ems.microdata_success as f64 / ems.urls_with_html as f64 * 100.0
        );
        println!(
            "  Both: {}/{} ({:.1}%)",
            ems.both_success,
            ems.urls_with_html,
            ems.both_success as f64 / ems.urls_with_html as f64 * 100.0
        );
        println!(
            "  Neither: {}/{} ({:.1}%)",
            ems.neither_success,
            ems.urls_with_html,
            ems.neither_success as f64 / ems.urls_with_html as f64 * 100.0
        );
    } else {
        println!("  (no HTML fetched)");
    }

    // Print ingredient parsing stats
    let ips = &results.ingredient_stats;
    if ips.total_ingredients > 0 {
        println!();
        println!("Ingredient Parsing Stats:");
        println!("  Total ingredients: {}", ips.total_ingredients);

        let volume_attempted =
            ips.volume_converted + ips.volume_unknown_ingredient + ips.volume_already_has_weight;
        if volume_attempted > 0 {
            println!(
                "  Volume→weight converted: {}/{} ({:.1}%)",
                ips.volume_converted,
                volume_attempted,
                ips.volume_converted as f64 / volume_attempted as f64 * 100.0
            );
            println!(
                "  Unknown ingredient (no density): {}",
                ips.volume_unknown_ingredient
            );
        }
        println!("  Already has weight: {}", ips.volume_already_has_weight);
        println!("  No volume unit: {}", ips.volume_no_volume);

        let metric_total = ips.metric_converted_oz + ips.metric_converted_lb;
        if metric_total > 0 {
            println!(
                "  Metric converted: {} oz→g, {} lb→g",
                ips.metric_converted_oz, ips.metric_converted_lb
            );
        }
    }

    println!();
    println!("Artifacts saved to: {}", run_dir.display());

    Ok(results)
}

// ============================================================================
// Helper functions
// ============================================================================

fn determine_final_status(steps: &[StepResult]) -> FinalStatus {
    for step in steps {
        if !step.success {
            match step.step {
                PipelineStep::FetchHtml => return FinalStatus::FailedAtFetch,
                PipelineStep::ExtractRecipe => return FinalStatus::FailedAtExtract,
                PipelineStep::SaveRecipe => return FinalStatus::FailedAtSave,
                PipelineStep::FetchImages | PipelineStep::ParseIngredients => {
                    // FetchImages is skipped in CLI, ParseIngredients runs before save
                    return FinalStatus::FailedAtSave;
                }
                PipelineStep::EnrichNormalizeIngredients
                | PipelineStep::EnrichAutoTag
                | PipelineStep::EnrichGeneratePhoto => {
                    // Enrichment failures are expected - don't fail the job
                    // Continue to check remaining steps
                }
            };
        }
    }
    FinalStatus::Completed
}

fn update_results(
    results: &mut PipelineResults,
    steps: &[StepResult],
    final_status: &FinalStatus,
    domain: &str,
    extraction_stats: Option<&ExtractionStats>,
    ingredient_stats: Option<&IngredientStats>,
    ai_cached: Option<bool>,
) {
    // Update HTML cache stats
    for step in steps {
        if step.step == PipelineStep::FetchHtml {
            if step.cached {
                results.cache_hits += 1;
            } else {
                results.cache_misses += 1;
            }
        }
    }

    // Update AI cache stats
    if let Some(cached) = ai_cached {
        if cached {
            results.ai_cache_hits += 1;
        } else {
            results.ai_cache_misses += 1;
        }
    }

    // Update overall stats
    match final_status {
        FinalStatus::Completed => results.completed += 1,
        FinalStatus::FailedAtFetch => results.failed_at_fetch += 1,
        FinalStatus::FailedAtExtract => results.failed_at_extract += 1,
        FinalStatus::FailedAtSave => results.failed_at_save += 1,
    }

    // Update extraction method stats
    // Count urls_with_html based on whether fetch succeeded (not just when extraction succeeds)
    let fetch_succeeded = steps
        .iter()
        .any(|s| s.step == PipelineStep::FetchHtml && s.success);

    if fetch_succeeded {
        results.extraction_method_stats.urls_with_html += 1;

        if let Some(stats) = extraction_stats {
            // We have extraction stats - count which methods worked
            if stats.jsonld_success {
                results.extraction_method_stats.jsonld_success += 1;
            }
            if stats.microdata_success {
                results.extraction_method_stats.microdata_success += 1;
            }
            if stats.jsonld_success && stats.microdata_success {
                results.extraction_method_stats.both_success += 1;
            }
            if !stats.jsonld_success && !stats.microdata_success {
                results.extraction_method_stats.neither_success += 1;
            }
        } else {
            // Fetch succeeded but no extraction stats - means extraction failed
            // This counts as "neither method succeeded"
            results.extraction_method_stats.neither_success += 1;
        }
    }

    // Update site stats
    let site_stats = results
        .by_site
        .entry(domain.to_string())
        .or_insert_with(|| SiteResults {
            domain: domain.to_string(),
            total: 0,
            completed: 0,
            failed: 0,
        });

    site_stats.total += 1;
    match final_status {
        FinalStatus::Completed => site_stats.completed += 1,
        _ => site_stats.failed += 1,
    }

    // Update ingredient parsing stats
    if let Some(stats) = ingredient_stats {
        results.ingredient_stats.total_ingredients += stats.total_ingredients;
        results.ingredient_stats.volume_converted += stats.volume_converted;
        results.ingredient_stats.volume_unknown_ingredient += stats.volume_unknown_ingredient;
        results.ingredient_stats.volume_no_volume += stats.volume_no_volume;
        results.ingredient_stats.volume_already_has_weight += stats.volume_already_has_weight;
        results.ingredient_stats.metric_converted_oz += stats.metric_converted_oz;
        results.ingredient_stats.metric_converted_lb += stats.metric_converted_lb;
    }
}

fn save_manifest(run_dir: &Path, manifest: &RunManifest) -> Result<()> {
    let json = serde_json::to_string_pretty(manifest)?;
    fs::write(run_dir.join("manifest.json"), json)?;
    Ok(())
}

fn save_results(run_dir: &Path, results: &PipelineResults) -> Result<()> {
    let json = serde_json::to_string_pretty(results)?;
    fs::write(run_dir.join("results.json"), json)?;
    Ok(())
}

fn truncate_url(url: &str, max_len: usize) -> String {
    if url.chars().count() <= max_len {
        url.to_string()
    } else {
        format!("{}...", url.chars().take(max_len - 3).collect::<String>())
    }
}

// ============================================================================
// Interactive cache prompting
// ============================================================================

/// Prompt user to manually save HTML for a URL, wait for file, and retry pipeline
async fn prompt_for_manual_cache(
    url: &str,
    client: Arc<CachingClient>,
    run_dir: &Path,
    user_tags: Vec<String>,
) -> Result<Option<AllStepsResult>> {
    let staging = staging_dir();

    println!();
    println!("  ┌─────────────────────────────────────────────────────────────┐");
    println!("  │ Fetch failed - manual cache needed                          │");
    println!("  └─────────────────────────────────────────────────────────────┘");
    println!();
    println!("  URL: {}", url);
    println!();
    println!("  To cache this page:");
    println!("  1. Open the URL above in your browser");
    println!("  2. Save the page (Cmd+S / Ctrl+S) to:");
    println!("     {}", staging.display());
    println!();
    print!("  Waiting for .html file... (or type 'skip' + Enter): ");
    io::stdout().flush()?;

    // Clear any existing files in staging
    clear_staging()?;

    // Use a channel to communicate between stdin reader and file watcher
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1);

    // Spawn a blocking task to read stdin
    let stdin_handle = tokio::task::spawn_blocking(move || {
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_ok() {
            let _ = tx.blocking_send(line);
        }
    });

    // Poll for file while waiting for stdin
    let poll_interval = Duration::from_millis(200);

    loop {
        // Check for file
        if let Some(staged_file) = find_staged_html() {
            // Wait a moment for write to complete
            tokio::time::sleep(Duration::from_millis(300)).await;

            // Abort the stdin task
            stdin_handle.abort();

            // Import the file
            println!();
            println!("  Found: {}", staged_file.display());

            // Read the HTML and inject into cache
            match fs::read_to_string(&staged_file) {
                Ok(html) => {
                    if let Err(e) = client.inject_html(url, &html) {
                        tracing::warn!(error = %e, "Failed to cache HTML");
                        println!("  Failed to cache: {}", e);
                        println!("  Continuing with failed status...");
                        println!();
                        return Ok(None);
                    }

                    // Remove the staged file
                    let _ = fs::remove_file(&staged_file);

                    println!("  Cached successfully, retrying pipeline...");
                    println!();

                    // Re-run all steps (should hit cache now)
                    let new_results =
                        run_all_steps(url, Arc::clone(&client), run_dir, false, user_tags).await;
                    return Ok(Some(new_results));
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to read staged HTML file");
                    println!("  Failed to read file: {}", e);
                    println!("  Continuing with failed status...");
                    println!();
                    return Ok(None);
                }
            }
        }

        // Check if user typed something
        match rx.try_recv() {
            Ok(line) => {
                if line.trim().eq_ignore_ascii_case("skip") || line.trim().is_empty() {
                    println!();
                    println!("  Skipped by user");
                    println!();
                    return Ok(None);
                }
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                // No input yet, continue waiting
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                // Stdin closed, skip
                tracing::warn!("Stdin closed, skipping URL");
                return Ok(None);
            }
        }

        tokio::time::sleep(poll_interval).await;
    }
}

// ============================================================================
// Cache stats command
// ============================================================================

pub fn print_cache_stats(cache_dir: &Path) {
    let cache = DiskCache::new(cache_dir.to_path_buf());
    let stats = cache.stats();

    println!("HTTP Cache Statistics");
    println!("=====================");
    println!("Cache directory: {}", cache_dir.display());
    println!("Cached responses (success): {}", stats.cached_success);
    println!("Cached errors: {}", stats.cached_errors);
    println!(
        "Total entries: {}",
        stats.cached_success + stats.cached_errors
    );
}

pub fn clear_cache(cache_dir: &Path) -> Result<()> {
    let cache = DiskCache::new(cache_dir.to_path_buf());
    cache.clear()?;
    println!("Cache cleared: {}", cache_dir.display());
    Ok(())
}

// ============================================================================
// Summary report generation
// ============================================================================

/// Get the path to the most recent pipeline run directory
pub fn get_latest_run_dir(output_dir: &Path) -> Result<(String, PathBuf)> {
    if !output_dir.exists() {
        anyhow::bail!(
            "Pipeline runs directory '{}' not found. Run `make pipeline` first.",
            output_dir.display()
        );
    }

    let mut runs: Vec<_> = fs::read_dir(output_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();

    runs.sort_by_key(|e| e.file_name());
    runs.reverse();

    let latest = runs
        .first()
        .ok_or_else(|| anyhow::anyhow!("No pipeline runs found in {}", output_dir.display()))?;

    let run_id = latest.file_name().to_string_lossy().to_string();
    Ok((run_id, latest.path()))
}

/// Output from the auto-tag step
#[derive(Debug, Deserialize)]
struct AutoTagOutput {
    suggested_tags: Vec<String>,
    cached: bool,
}

/// Generate a report of auto-tag suggestions from a pipeline run
pub fn generate_tag_report(run_dir: &Path) -> Result<String> {
    let mut report = String::new();
    report.push_str("# Auto-Tag Evaluation Report\n\n");

    let urls_dir = run_dir.join("urls");
    if !urls_dir.exists() {
        return Ok(report + "No URL results found.\n");
    }

    let mut entries: Vec<_> = fs::read_dir(&urls_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut total_with_tags = 0;
    let mut total_cached = 0;
    let mut tag_counts: HashMap<String, usize> = HashMap::new();

    report.push_str("## Per-Recipe Results\n\n");
    report.push_str("| Recipe | Tags | Cached |\n");
    report.push_str("|--------|------|--------|\n");

    for entry in &entries {
        let url_slug = entry.file_name().to_string_lossy().to_string();

        // Read extract_recipe output to get title
        let extract_path = entry.path().join("extract_recipe").join("output.json");
        let title = if extract_path.exists() {
            fs::read_to_string(&extract_path)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .and_then(|v| {
                    v.get("raw_recipe")?
                        .get("title")?
                        .as_str()
                        .map(String::from)
                })
                .unwrap_or_else(|| url_slug.clone())
        } else {
            url_slug.clone()
        };

        // Read auto-tag output
        let tag_path = entry.path().join("enrich_auto_tag").join("output.json");
        if tag_path.exists() {
            if let Ok(content) = fs::read_to_string(&tag_path) {
                if let Ok(output) = serde_json::from_str::<AutoTagOutput>(&content) {
                    let tags_str = if output.suggested_tags.is_empty() {
                        "_none_".to_string()
                    } else {
                        output.suggested_tags.join(", ")
                    };

                    let cached_str = if output.cached { "yes" } else { "no" };

                    // Truncate title for table (character-safe)
                    let title_display = if title.chars().count() > 40 {
                        format!("{}...", title.chars().take(37).collect::<String>())
                    } else {
                        title.clone()
                    };

                    report.push_str(&format!(
                        "| {} | {} | {} |\n",
                        title_display, tags_str, cached_str
                    ));

                    if !output.suggested_tags.is_empty() {
                        total_with_tags += 1;
                    }
                    if output.cached {
                        total_cached += 1;
                    }

                    for tag in &output.suggested_tags {
                        *tag_counts.entry(tag.clone()).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    // Summary stats
    report.push_str("\n## Summary\n\n");
    report.push_str(&format!("- Total recipes processed: {}\n", entries.len()));
    report.push_str(&format!(
        "- Recipes with tag suggestions: {}\n",
        total_with_tags
    ));
    report.push_str(&format!("- Cached responses: {}\n", total_cached));

    // Tag frequency
    if !tag_counts.is_empty() {
        report.push_str("\n## Tag Frequency\n\n");
        report.push_str("| Tag | Count |\n");
        report.push_str("|-----|-------|\n");

        let mut sorted_tags: Vec<_> = tag_counts.iter().collect();
        sorted_tags.sort_by(|a, b| b.1.cmp(a.1));

        for (tag, count) in sorted_tags {
            report.push_str(&format!("| {} | {} |\n", tag, count));
        }
    }

    Ok(report)
}

/// Generate a stable, diffable summary report from pipeline results
pub fn generate_summary_report(results: &PipelineResults) -> String {
    let mut report = String::new();

    // Overall stats
    report.push_str("# Pipeline Extraction Report\n\n");
    report.push_str("## Overall\n\n");
    report.push_str(&format!("- Total URLs: {}\n", results.total_urls));
    report.push_str(&format!(
        "- Completed: {} ({:.1}%)\n",
        results.completed,
        pct(results.completed, results.total_urls)
    ));
    report.push_str(&format!(
        "- Failed at fetch: {} ({:.1}%)\n",
        results.failed_at_fetch,
        pct(results.failed_at_fetch, results.total_urls)
    ));
    report.push_str(&format!(
        "- Failed at extract: {} ({:.1}%)\n",
        results.failed_at_extract,
        pct(results.failed_at_extract, results.total_urls)
    ));

    // Extraction method stats
    let ems = &results.extraction_method_stats;
    if ems.urls_with_html > 0 {
        report.push_str("\n## Extraction Methods\n\n");
        report.push_str(&format!(
            "- JSON-LD: {}/{} ({:.1}%)\n",
            ems.jsonld_success,
            ems.urls_with_html,
            pct(ems.jsonld_success, ems.urls_with_html)
        ));
        report.push_str(&format!(
            "- Microdata: {}/{} ({:.1}%)\n",
            ems.microdata_success,
            ems.urls_with_html,
            pct(ems.microdata_success, ems.urls_with_html)
        ));
        report.push_str(&format!(
            "- Both: {}/{} ({:.1}%)\n",
            ems.both_success,
            ems.urls_with_html,
            pct(ems.both_success, ems.urls_with_html)
        ));
        report.push_str(&format!(
            "- Neither: {}/{} ({:.1}%)\n",
            ems.neither_success,
            ems.urls_with_html,
            pct(ems.neither_success, ems.urls_with_html)
        ));
    }

    // Ingredient parsing stats
    let ips = &results.ingredient_stats;
    if ips.total_ingredients > 0 {
        report.push_str("\n## Ingredient Parsing\n\n");
        report.push_str(&format!("- Total ingredients: {}\n", ips.total_ingredients));

        // Volume-to-weight conversion stats
        let volume_attempted =
            ips.volume_converted + ips.volume_unknown_ingredient + ips.volume_already_has_weight;
        if volume_attempted > 0 {
            report.push_str(&format!(
                "- Volume-to-weight converted: {}/{} ({:.1}%)\n",
                ips.volume_converted,
                volume_attempted,
                pct(ips.volume_converted, volume_attempted)
            ));
            report.push_str(&format!(
                "- Unknown ingredient (no density data): {}\n",
                ips.volume_unknown_ingredient
            ));
        }
        report.push_str(&format!(
            "- Already has weight: {}\n",
            ips.volume_already_has_weight
        ));
        report.push_str(&format!(
            "- No volume unit (count-based): {}\n",
            ips.volume_no_volume
        ));

        // Metric conversion stats
        let metric_total = ips.metric_converted_oz + ips.metric_converted_lb;
        if metric_total > 0 {
            report.push_str(&format!(
                "- Metric converted (oz→g): {}\n",
                ips.metric_converted_oz
            ));
            report.push_str(&format!(
                "- Metric converted (lb→g): {}\n",
                ips.metric_converted_lb
            ));
        }
    }

    // AI cache stats
    let ai_total = results.ai_cache_hits + results.ai_cache_misses;
    if ai_total > 0 {
        report.push_str("\n## AI Cache\n\n");
        report.push_str(&format!(
            "- Cache hits: {}/{} ({:.1}%)\n",
            results.ai_cache_hits,
            ai_total,
            pct(results.ai_cache_hits, ai_total)
        ));
        report.push_str(&format!(
            "- API calls: {}/{} ({:.1}%)\n",
            results.ai_cache_misses,
            ai_total,
            pct(results.ai_cache_misses, ai_total)
        ));
    }

    // Per-site results (sorted alphabetically for stable diffs)
    report.push_str("\n## By Site\n\n");
    report.push_str("| Site | Completed | Total | Rate |\n");
    report.push_str("|------|-----------|-------|------|\n");

    let mut sites: Vec<_> = results.by_site.values().collect();
    sites.sort_by(|a, b| a.domain.cmp(&b.domain));

    for site in &sites {
        report.push_str(&format!(
            "| {} | {} | {} | {:.1}% |\n",
            site.domain,
            site.completed,
            site.total,
            pct(site.completed, site.total)
        ));
    }

    // Failed URLs grouped by error type (sorted for stability)
    let mut failures_by_error: std::collections::BTreeMap<String, Vec<&str>> =
        std::collections::BTreeMap::new();

    for url_result in &results.url_results {
        if !matches!(url_result.final_status, FinalStatus::Completed) {
            // Find the error message from the failed step
            let error = url_result
                .steps
                .iter()
                .find(|s| !s.success)
                .and_then(|s| s.error.as_ref())
                .map(|e| simplify_error(e))
                .unwrap_or_else(|| "Unknown error".to_string());

            failures_by_error
                .entry(error)
                .or_default()
                .push(&url_result.url);
        }
    }

    if !failures_by_error.is_empty() {
        report.push_str("\n## Failed URLs by Error\n");

        for (error, urls) in &failures_by_error {
            report.push_str(&format!("\n### {} ({} URLs)\n\n", error, urls.len()));
            let mut sorted_urls: Vec<_> = urls.iter().collect();
            sorted_urls.sort();
            for url in sorted_urls {
                report.push_str(&format!("- {}\n", url));
            }
        }
    }

    report
}

fn pct(num: usize, denom: usize) -> f64 {
    if denom == 0 {
        0.0
    } else {
        num as f64 / denom as f64 * 100.0
    }
}

/// Simplify error messages for grouping (remove URL-specific parts)
fn simplify_error(error: &str) -> String {
    // Extract just the error type for grouping
    // Handle both new format ("No recipe found") and legacy ("No Recipe found in JSON-LD")
    if error.contains("No recipe found") || error.contains("No Recipe found") {
        "No recipe found".to_string()
    } else if error.contains("MissingField") {
        // Extract the field name
        // Safe: start/end are from .find() on ASCII patterns
        #[allow(clippy::string_slice)]
        if let Some(start) = error.find("MissingField(") {
            if let Some(end) = error[start..].find(')') {
                return error[start..start + end + 1].to_string();
            }
        }
        "MissingField".to_string()
    } else if error.contains("Cached error") {
        "Cached fetch error".to_string()
    } else if error.contains("Fetch failed") {
        "Fetch failed".to_string()
    } else {
        // Truncate long errors
        let truncated: String = error.chars().take(50).collect();
        if truncated.len() < error.len() {
            format!("{}...", truncated)
        } else {
            truncated
        }
    }
}
