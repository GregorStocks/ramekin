use crate::generate_test_urls::TestUrlsOutput;
use crate::pipeline::{run_all_steps, HtmlCache, PipelineStep, StepResult};
use crate::OnFetchFail;
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

// ============================================================================
// Configuration
// ============================================================================

pub struct OrchestratorConfig {
    pub test_urls_file: PathBuf,
    pub output_dir: PathBuf,
    pub cache_dir: PathBuf,
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
            cache_dir: crate::pipeline::HtmlCache::default_cache_dir(),
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

    // Initialize cache
    let cache = HtmlCache::new(config.cache_dir);

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
        HtmlCache::ensure_staging_dir()?;
        HtmlCache::clear_staging()?;
        println!(
            "Interactive mode: save HTML files to {}",
            HtmlCache::staging_dir().display()
        );
        println!();
    }

    // Process each URL
    for (idx, (url, domain)) in urls_to_process.iter().enumerate() {
        let progress = format!("[{}/{}]", idx + 1, total_urls);

        // Check if we need to fetch (for rate limiting purposes)
        let needs_fetch = config.force_fetch || !cache.is_cached(url);

        // Print progress
        if needs_fetch {
            println!("{} {} (fetching...)", progress, truncate_url(url, 60));
        } else {
            println!("{} {} (cached)", progress, truncate_url(url, 60));
        }

        // Run all steps
        let mut step_results = run_all_steps(url, &cache, &run_dir, config.force_fetch).await;

        // Check if fetch failed
        let fetch_failed = step_results
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
                        prompt_for_manual_cache(url, &cache, &run_dir).await?
                    {
                        step_results = new_results;
                    }
                    // If user skipped, step_results still has the failed fetch
                }
                OnFetchFail::Continue => {
                    // Default: just continue (already have failed result)
                }
            }
        }

        // Determine final status
        let final_status = determine_final_status(&step_results);

        // Update results
        update_results(&mut results, &step_results, &final_status, domain);

        // Record URL result
        results.url_results.push(UrlResult {
            url: url.clone(),
            site: domain.clone(),
            steps: step_results,
            final_status,
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

    for site in sites.iter().take(10) {
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
    if sites.len() > 10 {
        println!("  ... and {} more sites", sites.len() - 10);
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
            return match step.step {
                PipelineStep::FetchHtml => FinalStatus::FailedAtFetch,
                PipelineStep::ExtractRecipe => FinalStatus::FailedAtExtract,
                PipelineStep::SaveRecipe => FinalStatus::FailedAtSave,
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
    cache: &HtmlCache,
    run_dir: &Path,
) -> Result<Option<Vec<StepResult>>> {
    let staging_dir = HtmlCache::staging_dir();

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
    println!("     {}", staging_dir.display());
    println!();
    print!("  Waiting for .html file... (or type 'skip' + Enter): ");
    io::stdout().flush()?;

    // Clear any existing files in staging
    HtmlCache::clear_staging()?;

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
        if let Some(staged_file) = HtmlCache::find_staged_html() {
            // Wait a moment for write to complete
            tokio::time::sleep(Duration::from_millis(300)).await;

            // Abort the stdin task
            stdin_handle.abort();

            // Import the file
            println!();
            println!("  Found: {}", staged_file.display());
            match cache.import_staged_file(&staged_file, url) {
                Ok(()) => {
                    println!("  Cached successfully, retrying pipeline...");
                    println!();

                    // Re-run all steps (should hit cache now)
                    let new_results = run_all_steps(url, cache, run_dir, false).await;
                    return Ok(Some(new_results));
                }
                Err(e) => {
                    println!("  Failed to import: {}", e);
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
                println!();
                println!("  Stdin closed, skipping...");
                println!();
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
    let cache = HtmlCache::new(cache_dir.to_path_buf());
    let stats = cache.stats();

    println!("HTML Cache Statistics");
    println!("=====================");
    println!("Cache directory: {}", cache_dir.display());
    println!("Cached HTML (success): {}", stats.cached_success);
    println!("Cached errors: {}", stats.cached_errors);
    println!("Total entries: {}", stats.total());
}

pub fn clear_cache(cache_dir: &Path) -> Result<()> {
    let cache = HtmlCache::new(cache_dir.to_path_buf());
    cache.clear()?;
    println!("Cache cleared: {}", cache_dir.display());
    Ok(())
}
