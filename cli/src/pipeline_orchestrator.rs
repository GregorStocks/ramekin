use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::generate_test_urls::TestUrlsOutput;
use crate::pipeline::{
    clear_staging, ensure_staging_dir, find_staged_html, run_all_steps, staging_dir,
    AllStepsResult, ExtractionStats, PipelineStep, StepResult,
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
    pub force_fetch: bool,
    pub on_fetch_fail: OnFetchFail,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            test_urls_file: PathBuf::from("data/test-urls.json"),
            output_dir: PathBuf::from("data/pipeline-runs"),
            limit: None,
            site_filter: None,
            delay_ms: 1000,
            force_fetch: false,
            on_fetch_fail: OnFetchFail::Continue,
        }
    }
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
    pub force_fetch: bool,
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
    pub by_site: HashMap<String, SiteResults>,
    pub url_results: Vec<UrlResult>,
    pub extraction_method_stats: ExtractionMethodStats,
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
            force_fetch: config.force_fetch,
        },
        status: RunStatus::Running,
    };
    save_manifest(&run_dir, &manifest)?;

    // Initialize HTTP client with caching
    // The CachingClient uses RAMEKIN_HTTP_CACHE env var for cache directory
    // and handles rate limiting internally
    let client = Arc::new(
        CachingClient::builder()
            .rate_limit_ms(0) // We handle delay ourselves between URLs
            .build()
            .context("Failed to create HTTP client")?,
    );

    // Initialize results
    let mut results = PipelineResults {
        total_urls: urls_to_process.len(),
        ..Default::default()
    };

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
    if matches!(config.on_fetch_fail, OnFetchFail::Prompt) {
        ensure_staging_dir()?;
        clear_staging()?;
        println!(
            "Interactive mode: save HTML files to {}",
            staging_dir().display()
        );
        println!();
    }

    // Process each URL
    for (idx, (url, domain)) in urls_to_process.iter().enumerate() {
        let progress = format!("[{}/{}]", idx + 1, total_urls);

        // Check if we need to fetch (for rate limiting purposes)
        let needs_fetch = config.force_fetch || !client.is_cached(url);

        // Print progress
        if needs_fetch {
            println!("{} {} (fetching...)", progress, truncate_url(url, 60));
        } else {
            println!("{} {} (cached)", progress, truncate_url(url, 60));
        }

        // Run all steps
        let mut all_results =
            run_all_steps(url, Arc::clone(&client), &run_dir, config.force_fetch).await;

        // Check if fetch failed
        let fetch_failed = all_results
            .step_results
            .first()
            .map(|r| r.step == PipelineStep::FetchHtml && !r.success)
            .unwrap_or(false);

        if fetch_failed {
            match config.on_fetch_fail {
                OnFetchFail::Skip => {
                    println!("  -> Skipped (fetch failed)");
                    // Don't record this URL at all
                    continue;
                }
                OnFetchFail::Prompt => {
                    // Interactive mode: prompt user to save HTML
                    if let Some(new_results) =
                        prompt_for_manual_cache(url, Arc::clone(&client), &run_dir).await?
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

        // Update results
        update_results(
            &mut results,
            &all_results.step_results,
            &final_status,
            domain,
            all_results.extraction_stats.as_ref(),
        );

        // Record URL result
        results.url_results.push(UrlResult {
            url: url.clone(),
            site: domain.clone(),
            steps: all_results.step_results,
            final_status,
            extraction_stats: all_results.extraction_stats,
        });

        // Save intermediate results
        save_results(&run_dir, &results)?;

        // Rate limiting: only delay if we actually fetched
        if needs_fetch && idx < total_urls - 1 {
            tokio::time::sleep(Duration::from_millis(config.delay_ms)).await;
        }
    }

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
        rate_b.partial_cmp(&rate_a).unwrap()
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
                PipelineStep::FetchImages => {
                    // FetchImages is skipped in CLI, but handle it for completeness
                    return FinalStatus::FailedAtSave;
                }
                PipelineStep::Enrich => {
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
) {
    // Update cache stats
    for step in steps {
        if step.step == PipelineStep::FetchHtml {
            if step.cached {
                results.cache_hits += 1;
            } else {
                results.cache_misses += 1;
            }
        }
    }

    // Update overall stats
    match final_status {
        FinalStatus::Completed => results.completed += 1,
        FinalStatus::FailedAtFetch => results.failed_at_fetch += 1,
        FinalStatus::FailedAtExtract => results.failed_at_extract += 1,
        FinalStatus::FailedAtSave => results.failed_at_save += 1,
    }

    // Update extraction method stats if we have them
    if let Some(stats) = extraction_stats {
        results.extraction_method_stats.urls_with_html += 1;

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
    if url.len() <= max_len {
        url.to_string()
    } else {
        format!("{}...", &url[..max_len - 3])
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
                    let new_results = run_all_steps(url, Arc::clone(&client), run_dir, false).await;
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

/// Load results from the most recent pipeline run
pub fn load_latest_results(output_dir: &Path) -> Result<(String, PipelineResults)> {
    // Find the most recent run directory
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
    let results_path = latest.path().join("results.json");

    let content = fs::read_to_string(&results_path)
        .with_context(|| format!("Failed to read {}", results_path.display()))?;

    let results: PipelineResults = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", results_path.display()))?;

    Ok((run_id, results))
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
