use anyhow::{anyhow, Context, Result};
use chrono::Utc;
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
    SaveRecipe,
}

impl PipelineStep {
    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "fetch_html" => Ok(PipelineStep::FetchHtml),
            "extract_recipe" => Ok(PipelineStep::ExtractRecipe),
            "save_recipe" => Ok(PipelineStep::SaveRecipe),
            _ => Err(anyhow!(
                "Unknown step: {}. Valid steps: fetch_html, extract_recipe, save_recipe",
                s
            )),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PipelineStep::FetchHtml => "fetch_html",
            PipelineStep::ExtractRecipe => "extract_recipe",
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

// ============================================================================
// URL slugification
// ============================================================================

/// Convert a URL to a filesystem-safe slug.
/// e.g., "https://www.seriouseats.com/best-chili-recipe-123" -> "seriouseats-com_best-chili-recipe-123"
pub fn slugify_url(url: &str) -> String {
    let parsed = match url::Url::parse(url) {
        Ok(p) => p,
        Err(_) => return sanitize_for_filesystem(url),
    };

    let host = parsed
        .host_str()
        .unwrap_or("unknown")
        .trim_start_matches("www.");

    let path = parsed.path().trim_matches('/');

    // Combine host and path, replacing special chars
    let combined = if path.is_empty() {
        host.to_string()
    } else {
        format!("{}_{}", host, path)
    };

    sanitize_for_filesystem(&combined)
}

fn sanitize_for_filesystem(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c == '.' || c == '/' {
                '-'
            } else {
                '_'
            }
        })
        .collect::<String>()
        .chars()
        .take(200) // Limit length for filesystem compatibility
        .collect()
}

// ============================================================================
// HTML Cache
// ============================================================================

pub struct HtmlCache {
    cache_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedHtml {
    pub html: String,
    pub fetched_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedError {
    pub error: String,
    pub fetched_at: String,
}

impl HtmlCache {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    pub fn default_cache_dir() -> PathBuf {
        // Use ~/.ramekin/pipeline-cache/html to share cache across workspaces
        dirs::home_dir()
            .map(|h| h.join(".ramekin").join("pipeline-cache").join("html"))
            .unwrap_or_else(|| PathBuf::from("data/pipeline-cache/html"))
    }

    fn url_dir(&self, url: &str) -> PathBuf {
        self.cache_dir.join(slugify_url(url))
    }

    /// Check if HTML is cached for a URL (either success or error)
    pub fn is_cached(&self, url: &str) -> bool {
        let dir = self.url_dir(url);
        dir.join("output.html").exists() || dir.join("error.txt").exists()
    }

    /// Get cached HTML if it exists
    pub fn get(&self, url: &str) -> Option<CachedHtml> {
        let dir = self.url_dir(url);
        let html_path = dir.join("output.html");
        let timestamp_path = dir.join("fetched_at.txt");

        if html_path.exists() {
            let html = fs::read_to_string(&html_path).ok()?;
            let fetched_at =
                fs::read_to_string(&timestamp_path).unwrap_or_else(|_| "unknown".to_string());
            Some(CachedHtml { html, fetched_at })
        } else {
            None
        }
    }

    /// Get cached error if it exists
    pub fn get_error(&self, url: &str) -> Option<CachedError> {
        let dir = self.url_dir(url);
        let error_path = dir.join("error.txt");
        let timestamp_path = dir.join("fetched_at.txt");

        if error_path.exists() {
            let error = fs::read_to_string(&error_path).ok()?;
            let fetched_at =
                fs::read_to_string(&timestamp_path).unwrap_or_else(|_| "unknown".to_string());
            Some(CachedError { error, fetched_at })
        } else {
            None
        }
    }

    /// Fetch HTML from URL, using cache if available. Returns (html, was_cached).
    pub async fn fetch_or_get(&self, url: &str, force: bool) -> Result<(String, bool)> {
        // Check cache first (unless forced)
        if !force {
            if let Some(cached) = self.get(url) {
                return Ok((cached.html, true));
            }
            if let Some(cached_error) = self.get_error(url) {
                return Err(anyhow!("Cached error: {}", cached_error.error));
            }
        }

        // Fetch from network
        match ramekin_core::fetch_html(url).await {
            Ok(html) => {
                self.save_html(url, &html)?;
                Ok((html, false))
            }
            Err(e) => {
                self.save_error(url, &e.to_string())?;
                Err(anyhow!("Fetch failed: {}", e))
            }
        }
    }

    fn save_html(&self, url: &str, html: &str) -> Result<()> {
        let dir = self.url_dir(url);
        fs::create_dir_all(&dir)?;

        fs::write(dir.join("url.txt"), url)?;
        fs::write(dir.join("output.html"), html)?;
        fs::write(dir.join("fetched_at.txt"), Utc::now().to_rfc3339())?;

        // Remove any cached error if we succeeded
        let error_path = dir.join("error.txt");
        if error_path.exists() {
            let _ = fs::remove_file(error_path);
        }

        Ok(())
    }

    fn save_error(&self, url: &str, error: &str) -> Result<()> {
        let dir = self.url_dir(url);
        fs::create_dir_all(&dir)?;

        fs::write(dir.join("url.txt"), url)?;
        fs::write(dir.join("error.txt"), error)?;
        fs::write(dir.join("fetched_at.txt"), Utc::now().to_rfc3339())?;

        Ok(())
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let mut stats = CacheStats::default();

        if !self.cache_dir.exists() {
            return stats;
        }

        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                if entry.path().is_dir() {
                    if entry.path().join("output.html").exists() {
                        stats.cached_success += 1;
                    } else if entry.path().join("error.txt").exists() {
                        stats.cached_errors += 1;
                    }
                }
            }
        }

        stats
    }

    /// Clear all cached HTML
    pub fn clear(&self) -> Result<()> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir)?;
        }
        Ok(())
    }

    /// Get the staging directory path for manual HTML saves
    pub fn staging_dir() -> PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".ramekin").join("cache-staging"))
            .unwrap_or_else(|| PathBuf::from("data/cache-staging"))
    }

    /// Ensure staging directory exists and return its path
    pub fn ensure_staging_dir() -> Result<PathBuf> {
        let staging = Self::staging_dir();
        fs::create_dir_all(&staging)?;
        Ok(staging)
    }

    /// Find the newest .html file in staging directory
    pub fn find_staged_html() -> Option<PathBuf> {
        let staging = Self::staging_dir();
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

    /// Import a staged HTML file into the cache for a URL
    pub fn import_staged_file(&self, staged_path: &Path, url: &str) -> Result<()> {
        let html = fs::read_to_string(staged_path)
            .with_context(|| format!("Failed to read staged file: {}", staged_path.display()))?;

        // Save to cache
        let dir = self.url_dir(url);
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("url.txt"), url)?;
        fs::write(dir.join("output.html"), &html)?;
        fs::write(dir.join("fetched_at.txt"), Utc::now().to_rfc3339())?;

        // Remove any cached error
        let error_path = dir.join("error.txt");
        if error_path.exists() {
            let _ = fs::remove_file(error_path);
        }

        // Remove the staged file
        let _ = fs::remove_file(staged_path);

        Ok(())
    }

    /// Clear any existing files in staging directory
    pub fn clear_staging() -> Result<()> {
        let staging = Self::staging_dir();
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
}

#[derive(Debug, Default)]
pub struct CacheStats {
    pub cached_success: usize,
    pub cached_errors: usize,
}

impl CacheStats {
    pub fn total(&self) -> usize {
        self.cached_success + self.cached_errors
    }
}

// ============================================================================
// Step runners
// ============================================================================

/// Run the fetch_html step for a URL.
pub async fn run_fetch_html(url: &str, cache: &HtmlCache, force: bool) -> StepResult {
    let start = Instant::now();

    let result = cache.fetch_or_get(url, force).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok((_, cached)) => StepResult {
            step: PipelineStep::FetchHtml,
            success: true,
            duration_ms,
            error: None,
            cached,
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
pub fn run_extract_recipe(url: &str, cache: &HtmlCache, run_dir: &Path) -> StepResult {
    let start = Instant::now();
    let slug = slugify_url(url);
    let output_dir = run_dir.join("urls").join(&slug).join("extract_recipe");

    // Get HTML from cache
    let html = match cache.get(url) {
        Some(cached) => cached.html,
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

    // Extract recipe
    match ramekin_core::extract_recipe(&html, url) {
        Ok(raw_recipe) => {
            let duration_ms = start.elapsed().as_millis() as u64;

            // Save output
            if let Err(e) = save_extract_output(&output_dir, &raw_recipe, duration_ms) {
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

// ============================================================================
// Output saving helpers
// ============================================================================

fn save_extract_output(
    output_dir: &Path,
    raw_recipe: &ramekin_core::RawRecipe,
    duration_ms: u64,
) -> Result<()> {
    fs::create_dir_all(output_dir)?;

    let output = ramekin_core::ExtractRecipeOutput {
        raw_recipe: raw_recipe.clone(),
    };
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
pub async fn run_all_steps(
    url: &str,
    cache: &HtmlCache,
    run_dir: &Path,
    force_fetch: bool,
) -> Vec<StepResult> {
    let mut results = Vec::new();

    // Step 1: Fetch HTML
    let fetch_result = run_fetch_html(url, cache, force_fetch).await;
    let fetch_success = fetch_result.success;
    results.push(fetch_result);

    if !fetch_success {
        return results;
    }

    // Step 2: Extract Recipe
    let extract_result = run_extract_recipe(url, cache, run_dir);
    let extract_success = extract_result.success;
    results.push(extract_result);

    if !extract_success {
        return results;
    }

    // Step 3: Save Recipe
    let save_result = run_save_recipe(url, run_dir);
    results.push(save_result);

    results
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
            PipelineStep::from_str("save_recipe").unwrap(),
            PipelineStep::SaveRecipe
        );
        assert!(PipelineStep::from_str("invalid").is_err());
    }
}
