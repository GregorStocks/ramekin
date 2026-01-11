use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use quick_xml::de::from_str;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

// ============================================================================
// Output structures
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct TestUrlsOutput {
    pub generated_at: String,
    pub config: GenerationConfig,
    pub sites: Vec<SiteEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerationConfig {
    pub num_sites: usize,
    pub urls_per_site: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteEntry {
    pub domain: String,
    pub rank: usize,
    pub urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub source: UrlSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UrlSource {
    Sitemap,
    Homepage,
    Failed,
    Merged,
}

// ============================================================================
// Sitemap XML structures
// ============================================================================

#[derive(Debug, Deserialize)]
struct SitemapIndex {
    sitemap: Vec<SitemapEntry>,
}

#[derive(Debug, Deserialize)]
struct SitemapEntry {
    loc: String,
}

#[derive(Debug, Deserialize)]
struct Urlset {
    url: Vec<UrlEntry>,
}

#[derive(Debug, Deserialize)]
struct UrlEntry {
    loc: String,
    #[allow(dead_code)]
    lastmod: Option<String>,
}

// ============================================================================
// Site ranking data
// ============================================================================

#[derive(Debug)]
struct RankedSite {
    domain: String,
    rank: usize,
}

// ============================================================================
// Main entry point
// ============================================================================

pub async fn generate_test_urls(
    output: &Path,
    num_sites: usize,
    urls_per_site: usize,
    merge: bool,
) -> Result<()> {
    let client = build_client()?;

    // Load existing data if merging
    let existing_data = if merge && output.exists() {
        let content = std::fs::read_to_string(output)
            .with_context(|| format!("Failed to read existing file: {}", output.display()))?;
        let data: TestUrlsOutput =
            serde_json::from_str(&content).context("Failed to parse existing JSON")?;
        Some(data)
    } else {
        None
    };

    // Build map of existing sites for merging
    let existing_sites: HashMap<String, SiteEntry> = existing_data
        .as_ref()
        .map(|d| {
            d.sites
                .iter()
                .map(|s| (s.domain.clone(), s.clone()))
                .collect()
        })
        .unwrap_or_default();

    // Fetch site rankings from detailed.com
    println!("Fetching site rankings from detailed.com...");
    let ranked_sites = fetch_site_rankings(&client).await?;
    println!("Found {} ranked sites", ranked_sites.len());

    // Process sites
    let sites_to_process: Vec<_> = ranked_sites.into_iter().take(num_sites).collect();
    let mut results: Vec<SiteEntry> = Vec::new();

    for site in &sites_to_process {
        // Check if we already have enough URLs for this site
        if let Some(existing) = existing_sites.get(&site.domain) {
            if existing.urls.len() >= urls_per_site && existing.source != UrlSource::Failed {
                println!(
                    "[{}/{}] {} - already have {} URLs, skipping",
                    site.rank,
                    sites_to_process.len(),
                    site.domain,
                    existing.urls.len()
                );
                results.push(existing.clone());
                continue;
            }
        }

        println!(
            "[{}/{}] Processing {}...",
            site.rank,
            sites_to_process.len(),
            site.domain
        );

        let result = process_site(&client, site, urls_per_site).await;

        // Merge with existing if applicable
        let final_entry = if let Some(existing) = existing_sites.get(&site.domain) {
            merge_site_entries(existing, &result)
        } else {
            result
        };

        println!(
            "  -> {} URLs (source: {:?})",
            final_entry.urls.len(),
            final_entry.source
        );
        results.push(final_entry);

        // Rate limiting
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // If merging, include sites from existing data that aren't in current rankings
    if merge {
        for (domain, entry) in &existing_sites {
            if !results.iter().any(|r| r.domain == *domain) {
                println!("Preserving {} from previous run", domain);
                results.push(entry.clone());
            }
        }
    }

    // Sort by rank
    results.sort_by_key(|s| s.rank);

    // Build output
    let output_data = TestUrlsOutput {
        generated_at: Utc::now().to_rfc3339(),
        config: GenerationConfig {
            num_sites,
            urls_per_site,
        },
        sites: results,
    };

    // Write output
    let json = serde_json::to_string_pretty(&output_data)?;

    // Ensure parent directory exists
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(output, &json)?;
    println!(
        "\nWrote {} sites to {}",
        output_data.sites.len(),
        output.display()
    );

    // Summary stats
    let total_urls: usize = output_data.sites.iter().map(|s| s.urls.len()).sum();
    let failed_count = output_data
        .sites
        .iter()
        .filter(|s| s.source == UrlSource::Failed)
        .count();
    println!("Total URLs: {}, Failed sites: {}", total_urls, failed_count);

    Ok(())
}

// ============================================================================
// HTTP client
// ============================================================================

fn build_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (compatible; Ramekin/1.0; +https://ramekin.app)")
        .build()
        .context("Failed to build HTTP client")
}

// ============================================================================
// Site rankings from detailed.com
// ============================================================================

async fn fetch_site_rankings(client: &reqwest::Client) -> Result<Vec<RankedSite>> {
    let url = "https://detailed.com/food-blogs/";
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch rankings: HTTP {}",
            response.status()
        ));
    }

    let html = response.text().await?;
    parse_detailed_rankings(&html)
}

fn parse_detailed_rankings(html: &str) -> Result<Vec<RankedSite>> {
    let document = Html::parse_document(html);
    let link_selector = Selector::parse("a[href]").unwrap();

    let mut sites = Vec::new();
    let mut seen_domains: HashSet<String> = HashSet::new();
    let mut rank = 0;

    // Domains to skip (not food blogs, navigation, etc.)
    let skip_domains = [
        "detailed.com",
        "google.com",
        "facebook.com",
        "twitter.com",
        "instagram.com",
        "pinterest.com",
        "youtube.com",
        "seoblueprint.com",
        "linkedin.com",
        "x.com",
        "apple.com",
        "spotify.com",
        "reddit.com",
        "tiktok.com",
        "amazon.com",
    ];

    // Known food blog domains to validate against
    let known_food_blogs = [
        "thekitchn.com",
        "food52.com",
        "seriouseats.com",
        "simplyrecipes.com",
        "smittenkitchen.com",
        "budgetbytes.com",
        "minimalistbaker.com",
        "pinchofyum.com",
        "sallysbakingaddiction.com",
        "cookieandkate.com",
        "loveandlemons.com",
        "gimmesomeoven.com",
        "skinnytaste.com",
        "howsweeteats.com",
        "101cookbooks.com",
        "davidlebovitz.com",
        "thepioneerwoman.com",
        "allrecipes.com",
        "foodnetwork.com",
        "epicurious.com",
        "bonappetit.com",
        "delish.com",
        "tasty.co",
        "myrecipes.com",
        "eatingwell.com",
        "tasteofhome.com",
        "therecipecritic.com",
        "damndelicious.net",
        "cafedelites.com",
        "natashaskitchen.com",
        "onceuponachef.com",
        "downshiftology.com",
        "halfbakedharvest.com",
        "twopeasandtheirpod.com",
        "acouplecooks.com",
        "cookingclassy.com",
        "recipetineats.com",
        "bbcgoodfood.com",
        "foodandwine.com",
        "marthastewart.com",
        "kingarthurbaking.com",
        "spruceeats.com",
        "nomnompaleo.com",
        "wellplated.com",
        "iamafoodblog.com",
        "alexandracooks.com",
        "ohsheglows.com",
        "thestayathomechef.com",
    ];

    for element in document.select(&link_selector) {
        if let Some(href) = element.value().attr("href") {
            // Look for external links that look like blog domains
            if let Ok(parsed) = url::Url::parse(href) {
                if let Some(host) = parsed.host_str() {
                    // Skip non-blog domains
                    let should_skip = skip_domains.iter().any(|skip| host.contains(skip));
                    if should_skip {
                        continue;
                    }

                    // Normalize domain (remove www.)
                    let domain = host.trim_start_matches("www.").to_string();

                    // Only include each domain once
                    if seen_domains.insert(domain.clone()) {
                        rank += 1;
                        sites.push(RankedSite { domain, rank });
                    }
                }
            }
        }
    }

    // Filter to only known food blogs if we got a lot of junk
    if sites.len() > 100
        || sites
            .iter()
            .take(5)
            .all(|s| !known_food_blogs.contains(&s.domain.as_str()))
    {
        // Too many results or first entries aren't food blogs - filter to known list
        sites.retain(|s| known_food_blogs.contains(&s.domain.as_str()));

        // Re-rank
        for (i, site) in sites.iter_mut().enumerate() {
            site.rank = i + 1;
        }
    }

    if sites.is_empty() {
        // Fallback: use hardcoded list
        return Ok(known_food_blogs
            .iter()
            .enumerate()
            .map(|(i, domain)| RankedSite {
                domain: domain.to_string(),
                rank: i + 1,
            })
            .collect());
    }

    Ok(sites)
}

// ============================================================================
// Process a single site
// ============================================================================

async fn process_site(
    client: &reqwest::Client,
    site: &RankedSite,
    urls_per_site: usize,
) -> SiteEntry {
    // Try sitemap first
    match try_sitemap(client, &site.domain, urls_per_site).await {
        Ok(urls) if !urls.is_empty() => {
            return SiteEntry {
                domain: site.domain.clone(),
                rank: site.rank,
                urls,
                error: None,
                source: UrlSource::Sitemap,
            };
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!("  Sitemap failed: {}", e);
        }
    }

    // Try homepage fallback
    match try_homepage(client, &site.domain, urls_per_site).await {
        Ok(urls) if !urls.is_empty() => {
            return SiteEntry {
                domain: site.domain.clone(),
                rank: site.rank,
                urls,
                error: None,
                source: UrlSource::Homepage,
            };
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!("  Homepage fallback failed: {}", e);
        }
    }

    // Failed to get URLs
    SiteEntry {
        domain: site.domain.clone(),
        rank: site.rank,
        urls: vec![],
        error: Some("Could not find recipe URLs from sitemap or homepage".to_string()),
        source: UrlSource::Failed,
    }
}

// ============================================================================
// Sitemap fetching and parsing
// ============================================================================

async fn try_sitemap(
    client: &reqwest::Client,
    domain: &str,
    urls_per_site: usize,
) -> Result<Vec<String>> {
    let mut all_urls = Vec::new();

    // Step 1: Try robots.txt first to get ALL sitemaps (the authoritative source)
    let robots_sitemaps = fetch_robots_sitemaps(client, domain).await;
    let prioritized_sitemaps = prioritize_sitemaps(robots_sitemaps);

    if !prioritized_sitemaps.is_empty() {
        // Process sitemaps from robots.txt in priority order
        for sitemap_url in prioritized_sitemaps.iter().take(3) {
            if let Ok(content) = fetch_sitemap(client, sitemap_url).await {
                if let Ok(urls) =
                    extract_urls_from_sitemap_recursive(client, &content, domain, 2).await
                {
                    all_urls.extend(urls);
                    if all_urls.len() >= 100 {
                        break;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    } else {
        // Step 2: Fall back to /sitemap.xml if robots.txt had no sitemaps
        let sitemap_url = format!("https://{}/sitemap.xml", domain);
        let sitemap_content = fetch_sitemap(client, &sitemap_url).await?;
        all_urls.extend(
            extract_urls_from_sitemap_recursive(client, &sitemap_content, domain, 2).await?,
        );
    }

    // Filter for recipe URLs and take the requested amount
    let recipe_urls: Vec<String> = all_urls
        .into_iter()
        .filter(|url| is_recipe_url(url))
        .take(urls_per_site)
        .collect();

    Ok(recipe_urls)
}

async fn fetch_sitemap(client: &reqwest::Client, url: &str) -> Result<String> {
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow!("HTTP {}", response.status()));
    }

    // Check if gzipped (need to extract before consuming response)
    let content_encoding = response
        .headers()
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_default();

    let bytes = response.bytes().await?;

    if content_encoding.contains("gzip") || url.ends_with(".gz") {
        // Decompress gzip
        use flate2::read::GzDecoder;
        use std::io::Read;

        let mut decoder = GzDecoder::new(&bytes[..]);
        let mut content = String::new();
        decoder.read_to_string(&mut content)?;
        Ok(content)
    } else {
        String::from_utf8(bytes.to_vec()).context("Invalid UTF-8 in sitemap")
    }
}

/// Safely parse a sitemap, catching panics from malformed XML.
/// Some sitemaps cause quick-xml to panic due to overflow issues.
fn safe_parse_sitemap_index(content: &str) -> Option<SitemapIndex> {
    std::panic::catch_unwind(|| from_str::<SitemapIndex>(content))
        .ok()
        .and_then(|r| r.ok())
}

/// Safely parse a urlset, catching panics from malformed XML.
fn safe_parse_urlset(content: &str) -> Option<Urlset> {
    std::panic::catch_unwind(|| from_str::<Urlset>(content))
        .ok()
        .and_then(|r| r.ok())
}

/// Recursively extract URLs from a sitemap, following sitemap indexes up to max_depth levels
async fn extract_urls_from_sitemap_recursive(
    client: &reqwest::Client,
    content: &str,
    domain: &str,
    max_depth: usize,
) -> Result<Vec<String>> {
    let mut urls = Vec::new();

    // Try to parse as sitemap index first (using safe parsing to handle malformed XML)
    if let Some(index) = safe_parse_sitemap_index(content) {
        if max_depth == 0 {
            return Err(anyhow!("Max sitemap depth reached"));
        }

        // Use the same prioritization logic as robots.txt processing
        let all_sitemaps: Vec<String> = index.sitemap.iter().map(|s| s.loc.clone()).collect();
        let prioritized = prioritize_sitemaps(all_sitemaps);

        // Fetch and parse sub-sitemaps (limit to 3 to avoid too many requests)
        for sitemap_url in prioritized.iter().take(3) {
            if let Ok(sub_content) = fetch_sitemap(client, sitemap_url).await {
                if let Ok(sub_urls) = Box::pin(extract_urls_from_sitemap_recursive(
                    client,
                    &sub_content,
                    domain,
                    max_depth - 1,
                ))
                .await
                {
                    urls.extend(sub_urls);
                    // Stop if we have enough URLs
                    if urls.len() >= 100 {
                        break;
                    }
                }
            }
            // Small delay between sitemap fetches
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        return Ok(urls);
    }

    // Try to parse as urlset (using safe parsing to handle malformed XML)
    if let Some(urlset) = safe_parse_urlset(content) {
        for entry in urlset.url {
            // Verify URL is from the same domain
            if let Ok(parsed) = url::Url::parse(&entry.loc) {
                if let Some(host) = parsed.host_str() {
                    let normalized = host.trim_start_matches("www.");
                    if normalized == domain || domain.ends_with(normalized) {
                        urls.push(entry.loc);
                    }
                }
            }
        }
    }

    Ok(urls)
}

/// Fetch robots.txt and extract ALL sitemap URLs
async fn fetch_robots_sitemaps(client: &reqwest::Client, domain: &str) -> Vec<String> {
    let robots_url = format!("https://{}/robots.txt", domain);

    let response = match client.get(&robots_url).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => return Vec::new(),
    };

    let content = match response.text().await {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    parse_robots_sitemaps(&content)
}

/// Parse ALL sitemap URLs from robots.txt content
fn parse_robots_sitemaps(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.to_lowercase().starts_with("sitemap:") {
                Some(line[8..].trim().to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Prioritize and filter sitemaps deterministically.
/// Excludes category/tag/page/author sitemaps.
/// Prioritizes recipe > post > others, then sorts alphabetically for determinism.
fn prioritize_sitemaps(mut sitemaps: Vec<String>) -> Vec<String> {
    // Exclude sitemaps that contain index/category pages
    let excluded = ["category", "tag", "page", "author"];

    sitemaps.retain(|s| {
        let lower = s.to_lowercase();
        !excluded.iter().any(|ex| lower.contains(ex))
    });

    // Sort deterministically: priority first, then alphabetically by URL
    sitemaps.sort_by(|a, b| {
        let priority = |s: &str| -> u8 {
            let lower = s.to_lowercase();
            if lower.contains("recipe") {
                0
            } else if lower.contains("post") {
                1
            } else {
                2
            }
        };
        priority(a).cmp(&priority(b)).then_with(|| a.cmp(b))
    });

    sitemaps
}

// ============================================================================
// Homepage fallback
// ============================================================================

async fn try_homepage(
    client: &reqwest::Client,
    domain: &str,
    urls_per_site: usize,
) -> Result<Vec<String>> {
    let homepage_url = format!("https://{}/", domain);
    let response = client.get(&homepage_url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow!("HTTP {}", response.status()));
    }

    let html = response.text().await?;
    let document = Html::parse_document(&html);
    let link_selector = Selector::parse("a[href]").unwrap();

    let mut recipe_urls = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for element in document.select(&link_selector) {
        if let Some(href) = element.value().attr("href") {
            // Resolve relative URLs
            let full_url = if href.starts_with("http") {
                href.to_string()
            } else if href.starts_with('/') {
                format!("https://{}{}", domain, href)
            } else {
                continue;
            };

            // Check if it's a recipe URL from this domain
            if let Ok(parsed) = url::Url::parse(&full_url) {
                if let Some(host) = parsed.host_str() {
                    let normalized = host.trim_start_matches("www.");
                    if normalized == domain
                        && is_recipe_url(&full_url)
                        && seen.insert(full_url.clone())
                    {
                        recipe_urls.push(full_url);
                        if recipe_urls.len() >= urls_per_site {
                            break;
                        }
                    }
                }
            }
        }
    }

    Ok(recipe_urls)
}

// ============================================================================
// Recipe URL detection
// ============================================================================

use regex::Regex;
use std::sync::LazyLock;

/// Detect roundup/collection pages that aggregate multiple recipes
fn is_roundup_url(url: &str) -> bool {
    static ROUNDUP_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        vec![
            // "30-easy-recipes", "15-chicken-recipes", "100-best-recipes"
            Regex::new(r"/\d+-[a-z]+-recipes?(/|$)").unwrap(),
            // "top-10-recipes", "top-25-most-popular"
            Regex::new(r"/top-\d+").unwrap(),
            // "best-25-soup-recipes"
            Regex::new(r"/best-\d+").unwrap(),
            // "recipe-roundup", "christmas-recipe-round-up"
            Regex::new(r"-(roundup|round-up)(/|$)").unwrap(),
            // "recipes-of-2021", "favorite-recipes-of-2009"
            Regex::new(r"/[a-z]+-recipes?-of-\d{4}").unwrap(),
            // "our-10-favorite-recipes"
            Regex::new(r"/our-\d+-favorite").unwrap(),
            // "all-recipes/" at end
            Regex::new(r"/all-recipes/?$").unwrap(),
            // Recipe category indexes like "/chicken-recipes/" at end of path
            Regex::new(r"/[a-z]+-recipes/?$").unwrap(),
            // "50-healthy-recipes-to-kick-off"
            Regex::new(r"/\d+-[a-z]+-recipes-to-").unwrap(),
        ]
    });

    ROUNDUP_PATTERNS.iter().any(|re| re.is_match(url))
}

/// Filter out very old posts that predate structured recipe data (JSON-LD/Microdata)
fn is_very_old_post(url: &str) -> bool {
    static DATE_PATTERN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"/(\d{4})/\d{2}/").unwrap());

    if let Some(caps) = DATE_PATTERN.captures(url) {
        if let Some(year_str) = caps.get(1) {
            if let Ok(year) = year_str.as_str().parse::<u32>() {
                // Posts before 2010 often lack structured recipe data
                return year < 2010;
            }
        }
    }
    false
}

/// Check if URL is a dated blog post from a recent year (2010+)
fn is_recent_dated_post(url: &str) -> bool {
    // Pattern: /YYYY/MM/slug or /YYYY/MM/DD/slug (with optional day)
    static DATED_POST_PATTERN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"/20[1-9]\d/\d{2}/(\d{2}/)?[a-z0-9-]{5,}").unwrap());

    DATED_POST_PATTERN.is_match(url)
}

fn is_recipe_url(url: &str) -> bool {
    let lower = url.to_lowercase();

    // === PHASE 1: SITE-SPECIFIC LOGIC ===
    // Handle sites with unique URL structures

    if lower.contains("seriouseats.com") {
        // SeriousEats: reject collection pages (plural "-recipes-")
        if lower.contains("-recipes-") {
            return false;
        }
        // Require the singular "-recipe-" pattern
        return lower.contains("-recipe-");
    }

    // === PHASE 2: UNIVERSAL EXCLUSIONS ===
    // Reject URLs that are clearly NOT individual recipes

    // Category and tag pages
    if lower.contains("/category/")
        || lower.contains("/tag/")
        || lower.contains("/page/")
        || lower.contains("/author/")
    {
        return false;
    }

    // Static/info pages
    if lower.contains("/about")
        || lower.contains("/contact")
        || lower.contains("/privacy")
        || lower.contains("/terms")
    {
        return false;
    }

    // Malformed relative paths (e.g., smittenkitchen ./recipes/)
    if lower.contains("/./") {
        return false;
    }

    // URLs with fragment identifiers (comments sections)
    if lower.contains("#comments") || lower.contains("#respond") {
        return false;
    }

    // Roundup/collection patterns
    if is_roundup_url(&lower) {
        return false;
    }

    // Very old posts (before 2010) - often lack structured data
    if is_very_old_post(&lower) {
        return false;
    }

    // === PHASE 3: POSITIVE INDICATORS ===
    // Check for patterns that suggest individual recipes

    // Strong positive: singular "/recipe/" in path
    if lower.contains("/recipe/") {
        return true;
    }

    // Check "/recipes/" followed by a slug (not ending there)
    if let Some(pos) = lower.find("/recipes/") {
        let after_recipes = &lower[pos + 9..];
        // Strip trailing slash if present
        let slug = after_recipes.trim_end_matches('/');
        // Must have content after /recipes/ and not be a category path or nested
        if !slug.is_empty()
            && !slug.starts_with("category")
            && !slug.contains('/')
            && slug.len() > 3
        {
            return true;
        }
    }

    // "-recipe" or "-recipe/" suffix (singular, specific recipe)
    // But NOT "-recipes" (plural = collection)
    if (lower.contains("-recipe/") || lower.ends_with("-recipe")) && !lower.contains("-recipes") {
        return true;
    }

    // Date-based blog URLs from recent years with descriptive slugs
    if is_recent_dated_post(&lower) {
        return true;
    }

    false
}

// ============================================================================
// Merging
// ============================================================================

fn merge_site_entries(existing: &SiteEntry, new: &SiteEntry) -> SiteEntry {
    // Union URLs
    let mut all_urls: HashSet<String> = existing.urls.iter().cloned().collect();
    all_urls.extend(new.urls.iter().cloned());

    let urls: Vec<String> = all_urls.into_iter().collect();
    let is_empty = urls.is_empty();

    // Use the better source
    let source = if !new.urls.is_empty() && new.source != UrlSource::Failed {
        if existing.source == UrlSource::Merged || !existing.urls.is_empty() {
            UrlSource::Merged
        } else {
            new.source.clone()
        }
    } else if !existing.urls.is_empty() {
        existing.source.clone()
    } else {
        UrlSource::Failed
    };

    SiteEntry {
        domain: new.domain.clone(),
        rank: new.rank,
        urls,
        error: if is_empty {
            new.error.clone().or_else(|| existing.error.clone())
        } else {
            None
        },
        source,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_recipe_url_basic() {
        // Positive cases - individual recipes
        assert!(is_recipe_url("https://example.com/recipe/chocolate-cake"));
        assert!(is_recipe_url("https://example.com/recipes/minestrone-soup"));
        assert!(is_recipe_url("https://example.com/chocolate-cake-recipe/"));
        assert!(is_recipe_url(
            "https://example.com/2025/01/my-delicious-recipe"
        ));

        // Negative cases - static pages
        assert!(!is_recipe_url("https://example.com/about"));
        assert!(!is_recipe_url("https://example.com/category/desserts"));
        assert!(!is_recipe_url("https://example.com/tag/chocolate"));
    }

    #[test]
    fn test_roundup_detection() {
        // Roundup patterns should be rejected
        assert!(!is_recipe_url("https://example.com/30-easy-recipes/"));
        assert!(!is_recipe_url(
            "https://example.com/top-10-best-soup-recipes/"
        ));
        assert!(!is_recipe_url(
            "https://example.com/christmas-recipe-roundup/"
        ));
        assert!(!is_recipe_url(
            "https://example.com/favorite-recipes-of-2021/"
        ));
        assert!(!is_recipe_url(
            "https://twopeasandtheirpod.com/our-10-favorite-recipes-from-2011/"
        ));
        assert!(!is_recipe_url(
            "https://example.com/50-healthy-recipes-to-kick-off-2012/"
        ));
        assert!(!is_recipe_url(
            "https://example.com/healthy-dinner-recipes/"
        ));
        assert!(!is_recipe_url("https://example.com/all-recipes/"));

        // But individual recipes with numbers should be accepted
        assert!(is_recipe_url(
            "https://example.com/5-ingredient-pasta-recipe/"
        ));
        assert!(is_recipe_url(
            "https://example.com/20-minute-chicken-recipe/"
        ));
    }

    #[test]
    fn test_category_exclusion() {
        // Category pages should be rejected
        assert!(!is_recipe_url(
            "https://davidlebovitz.com/category/recipes/breads/"
        ));
        assert!(!is_recipe_url(
            "https://example.com/category/recipes/desserts/"
        ));

        // Malformed paths should be rejected
        assert!(!is_recipe_url(
            "https://smittenkitchen.com/./recipes/vegetable/cabbage/"
        ));

        // Comment fragment URLs should be rejected
        assert!(!is_recipe_url("https://example.com/2025/01/cake/#comments"));
    }

    #[test]
    fn test_old_post_filtering() {
        // Very old posts (pre-2010) should be rejected
        assert!(!is_recipe_url(
            "https://bakingbites.com/2004/12/chocolate-cake/"
        ));
        assert!(!is_recipe_url(
            "https://howsweeteats.com/2009/09/random-post/"
        ));
        assert!(!is_recipe_url("https://example.com/2005/01/old-recipe/"));

        // Recent dated posts should be accepted
        assert!(is_recipe_url("https://example.com/2023/01/chocolate-cake/"));
        assert!(is_recipe_url(
            "https://alexandracooks.com/2019/06/21/fish-en-papillote/"
        ));
    }

    #[test]
    fn test_valid_recipe_url_patterns() {
        // /recipe/ path pattern
        assert!(is_recipe_url(
            "https://altonbrown.com/recipes/beef-carpaccio/"
        ));
        assert!(is_recipe_url(
            "https://slenderkitchen.com/recipe/salmon-burgers"
        ));

        // -recipe suffix pattern
        assert!(is_recipe_url(
            "https://therecipecritic.com/avocado-toast-recipe/"
        ));
        assert!(is_recipe_url("https://cookingclassy.com/brownie-recipe/"));
    }

    #[test]
    fn test_seriouseats_recipe_url_filtering() {
        // Individual recipes should be accepted (singular "-recipe-" pattern)
        assert!(is_recipe_url(
            "https://www.seriouseats.com/hot-milk-cake-recipe-11878680"
        ));
        assert!(is_recipe_url(
            "https://www.seriouseats.com/lentil-sausage-stew-recipe-11880797"
        ));
        assert!(is_recipe_url(
            "https://www.seriouseats.com/binakol-na-manok-filipino-chicken-and-coconut-soup-recipe-11878784"
        ));

        // Collection pages should be rejected (plural "-recipes-" pattern)
        assert!(!is_recipe_url(
            "https://www.seriouseats.com/vegan-dinner-recipes-11878691"
        ));
        assert!(!is_recipe_url(
            "https://www.seriouseats.com/tahini-recipes-beyond-hummus-11878978"
        ));
        assert!(!is_recipe_url(
            "https://www.seriouseats.com/most-saved-shrimp-recipes-11879657"
        ));
        assert!(!is_recipe_url(
            "https://www.seriouseats.com/hearty-chickpea-recipes-11878318"
        ));
    }

    #[test]
    fn test_parse_urlset() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/recipe/cake</loc>
    <lastmod>2025-01-01</lastmod>
  </url>
  <url>
    <loc>https://example.com/recipe/cookies</loc>
  </url>
</urlset>"#;

        // Parse directly as urlset (sync version for testing)
        let urlset: Urlset = from_str(xml).unwrap();
        let urls: Vec<String> = urlset.url.iter().map(|u| u.loc.clone()).collect();

        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&"https://example.com/recipe/cake".to_string()));
        assert!(urls.contains(&"https://example.com/recipe/cookies".to_string()));
    }

    #[test]
    fn test_parse_robots_sitemaps() {
        let content = r#"
User-agent: *
Disallow: /admin/

Sitemap: https://example.com/post-sitemap.xml
Sitemap: https://example.com/category-sitemap.xml
Sitemap: https://example.com/recipe-sitemap.xml
"#;

        let sitemaps = parse_robots_sitemaps(content);
        assert_eq!(sitemaps.len(), 3);
        assert!(sitemaps.contains(&"https://example.com/post-sitemap.xml".to_string()));
        assert!(sitemaps.contains(&"https://example.com/category-sitemap.xml".to_string()));
        assert!(sitemaps.contains(&"https://example.com/recipe-sitemap.xml".to_string()));
    }

    #[test]
    fn test_prioritize_sitemaps_filters_and_sorts() {
        let sitemaps = vec![
            "https://example.com/category-sitemap.xml".to_string(),
            "https://example.com/post-sitemap.xml".to_string(),
            "https://example.com/tag-sitemap.xml".to_string(),
            "https://example.com/recipe-sitemap.xml".to_string(),
            "https://example.com/page-sitemap.xml".to_string(),
            "https://example.com/sitemap-1.xml".to_string(),
        ];

        let prioritized = prioritize_sitemaps(sitemaps);

        // Should exclude category, tag, page
        assert_eq!(prioritized.len(), 3);

        // Should be sorted: recipe first, then post, then others
        assert_eq!(prioritized[0], "https://example.com/recipe-sitemap.xml");
        assert_eq!(prioritized[1], "https://example.com/post-sitemap.xml");
        assert_eq!(prioritized[2], "https://example.com/sitemap-1.xml");
    }

    #[test]
    fn test_prioritize_sitemaps_deterministic() {
        // Multiple sitemaps with same priority should be sorted alphabetically
        let sitemaps = vec![
            "https://example.com/z-sitemap.xml".to_string(),
            "https://example.com/a-sitemap.xml".to_string(),
            "https://example.com/m-sitemap.xml".to_string(),
        ];

        let prioritized = prioritize_sitemaps(sitemaps.clone());

        // All have priority 2 (others), so should be alphabetically sorted
        assert_eq!(prioritized[0], "https://example.com/a-sitemap.xml");
        assert_eq!(prioritized[1], "https://example.com/m-sitemap.xml");
        assert_eq!(prioritized[2], "https://example.com/z-sitemap.xml");

        // Should be deterministic on re-run
        let prioritized2 = prioritize_sitemaps(sitemaps);
        assert_eq!(prioritized, prioritized2);
    }
}
