use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use quick_xml::de::from_str;
use ramekin_core::http::{CachingClient, HttpClient};
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
    site_filter: Option<&str>,
    min_year: u32,
    no_limit: bool,
) -> Result<()> {
    // Use CachingClient with rate limiting to avoid hammering servers
    let client = CachingClient::new().context("Failed to create HTTP client")?;

    // When filtering to a single site, always merge to preserve other sites
    let merge = merge || site_filter.is_some();

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

    // Get list of known food blogs
    let ranked_sites = get_known_food_blogs();

    // Filter to specific site if requested
    let ranked_sites: Vec<_> = if let Some(site_domain) = site_filter {
        let filtered: Vec<_> = ranked_sites
            .into_iter()
            .filter(|s| s.domain == site_domain)
            .collect();
        if filtered.is_empty() {
            return Err(anyhow!("No sites matched filter: {}", site_domain));
        }
        filtered
    } else {
        ranked_sites
    };

    println!("Found {} known food blog sites", ranked_sites.len());

    // Process sites
    let sites_to_process: Vec<_> = ranked_sites.into_iter().take(num_sites).collect();
    let mut results: Vec<SiteEntry> = Vec::new();

    for site in &sites_to_process {
        // Check if we already have enough URLs for this site (skip when no_limit or site_filter)
        if !no_limit && site_filter.is_none() {
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
        }

        println!(
            "[{}/{}] Processing {}...",
            site.rank,
            sites_to_process.len(),
            site.domain
        );

        let result = process_site(&client, site, urls_per_site, min_year, no_limit).await;

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
// Refilter existing URLs
// ============================================================================

/// Refilter existing test-urls.json through current is_recipe_url() logic.
/// Strips query strings and fragments from URLs, deduplicates, and removes
/// URLs that no longer pass the filter. Does not fetch any new URLs.
pub fn refilter_test_urls(path: &Path, _min_year: u32) -> Result<()> {
    // Use min_year=0 to skip the time-based filter. Refiltering should only
    // apply structural filters (roundup detection, site-specific rules, etc.),
    // not reject old posts that were already accepted into the URL list.
    let min_year: u32 = 0;
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let mut data: TestUrlsOutput =
        serde_json::from_str(&content).context("Failed to parse existing JSON")?;

    let mut total_removed = 0;
    let mut total_deduped = 0;

    for site in &mut data.sites {
        let before = site.urls.len();

        // Step 1: Clean URLs (strip query strings and fragments)
        for url in site.urls.iter_mut() {
            let clean = url
                .split('?')
                .next()
                .unwrap_or(url)
                .split('#')
                .next()
                .unwrap_or(url)
                .to_string();
            *url = clean;
        }

        // Step 2: Deduplicate
        let mut seen = HashSet::new();
        site.urls.retain(|url| seen.insert(url.clone()));
        let after_dedup = site.urls.len();
        let deduped = before - after_dedup;

        // Step 3: Apply filter
        let mut removed = Vec::new();
        site.urls.retain(|url| {
            let keep = is_recipe_url(url, min_year);
            if !keep {
                removed.push(url.clone());
            }
            keep
        });

        for url in &removed {
            println!("  REMOVED [{}]: {}", site.domain, url);
        }
        if deduped > 0 {
            println!("  DEDUPED [{}]: {} duplicates", site.domain, deduped);
        }

        total_deduped += deduped;
        total_removed += removed.len();
    }

    let total_urls: usize = data.sites.iter().map(|s| s.urls.len()).sum();
    println!();
    println!("Deduped: {} URLs", total_deduped);
    println!("Removed: {} URLs", total_removed);
    println!("Total remaining: {}", total_urls);

    // Write back
    let json = serde_json::to_string_pretty(&data)?;
    std::fs::write(path, &json)?;
    println!("Wrote filtered URLs to {}", path.display());

    Ok(())
}

// ============================================================================
// Site list
// ============================================================================

fn get_known_food_blogs() -> Vec<RankedSite> {
    let known_food_blogs = [
        // Original 48 sites
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
        // High-traffic sites
        "spendwithpennies.com",
        "food.com",
        "tastesbetterfromscratch.com",
        // Popular blogs from Detailed.com
        "onmykidsplate.com",
        "browneyedbaker.com",
        "mybakingaddiction.com",
        "loveandoliveoil.com",
        "slenderkitchen.com",
        "ohmyveggies.com",
        "altonbrown.com",
        "dinnerthendessert.com",
        "sweetandsavorymeals.com",
        "bakingbites.com",
        "sprinklebakes.com",
        "spoonforkbacon.com",
        "chefspencil.com",
        // Japanese & Asian recipe sites
        "justonecookbook.com",
        "sudachirecipes.com",
        "norecipes.com",
        "japanesecooking101.com",
        "chopstickchronicles.com",
        "thewoksoflife.com",
        // Other well-known sites
        "indianhealthyrecipes.com",
        "inspiredtaste.net",
        "jocooks.com",
        "themediterraneandish.com",
        "joyfoodsunshine.com",
        "themodernproper.com",
        "hostthetoast.com",
        "keviniscooking.com",
        "whiteonricecouple.com",
        "asweetpeachef.com",
        "afamilyfeast.com",
        "barefeetinthekitchen.com",
        "littlesweetbaker.com",
        "bakerita.com",
        "feastingathome.com",
        "peasandcrayons.com",
        "closetcooking.com",
        "lecremedelacrumb.com",
        "theforkedspoon.com",
        "lifemadesweeter.com",
        "averiecooks.com",
        "butterwithasideofbread.com",
        "yellowblissroad.com",
        "gonnawantseconds.com",
        "momontimeout.com",
    ];

    known_food_blogs
        .iter()
        .enumerate()
        .map(|(i, domain)| RankedSite {
            domain: domain.to_string(),
            rank: i + 1,
        })
        .collect()
}

// ============================================================================
// Process a single site
// ============================================================================

async fn process_site(
    client: &CachingClient,
    site: &RankedSite,
    urls_per_site: usize,
    min_year: u32,
    no_limit: bool,
) -> SiteEntry {
    // Try sitemap first
    match try_sitemap(client, &site.domain, urls_per_site, min_year, no_limit).await {
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
            tracing::warn!(error = %e, "Sitemap failed");
        }
    }

    // Try homepage fallback
    match try_homepage(client, &site.domain, urls_per_site, min_year, no_limit).await {
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
            tracing::warn!(error = %e, "Homepage fallback failed");
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
    client: &CachingClient,
    domain: &str,
    urls_per_site: usize,
    min_year: u32,
    no_limit: bool,
) -> Result<Vec<String>> {
    let mut all_urls = Vec::new();

    // Step 1: Try robots.txt first to get ALL sitemaps (the authoritative source)
    let robots_sitemaps = fetch_robots_sitemaps(client, domain).await;
    let prioritized_sitemaps = prioritize_sitemaps(robots_sitemaps);

    let max_sitemaps = if no_limit { usize::MAX } else { 3 };
    let max_urls = if no_limit { usize::MAX } else { 200 };

    if !prioritized_sitemaps.is_empty() {
        // Process sitemaps from robots.txt in priority order
        for sitemap_url in prioritized_sitemaps.iter().take(max_sitemaps) {
            if let Ok(content) = fetch_sitemap(client, sitemap_url).await {
                if let Ok(urls) =
                    extract_urls_from_sitemap_recursive(client, &content, domain, 2, no_limit).await
                {
                    all_urls.extend(urls);
                    if all_urls.len() >= max_urls {
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
            extract_urls_from_sitemap_recursive(client, &sitemap_content, domain, 2, no_limit)
                .await?,
        );
    }

    // Filter for recipe URLs and take the requested amount
    let final_limit = if no_limit { usize::MAX } else { urls_per_site };
    let recipe_urls: Vec<String> = all_urls
        .into_iter()
        .filter(|url| is_recipe_url(url, min_year))
        .take(final_limit)
        .collect();

    Ok(recipe_urls)
}

async fn fetch_sitemap(client: &CachingClient, url: &str) -> Result<String> {
    let bytes = client
        .fetch_bytes(url)
        .await
        .map_err(|e| anyhow!("Failed to fetch sitemap: {}", e))?;

    // Check if content looks like gzip (magic bytes) or URL ends in .gz
    let is_gzip =
        (bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b) || url.ends_with(".gz");

    if is_gzip {
        // Decompress gzip
        use flate2::read::GzDecoder;
        use std::io::Read;

        let mut decoder = GzDecoder::new(&bytes[..]);
        let mut content = String::new();
        decoder.read_to_string(&mut content)?;
        Ok(content)
    } else {
        String::from_utf8(bytes).context("Invalid UTF-8 in sitemap")
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
    client: &CachingClient,
    content: &str,
    domain: &str,
    max_depth: usize,
    no_limit: bool,
) -> Result<Vec<String>> {
    let mut urls = Vec::new();

    let max_sitemaps = if no_limit { usize::MAX } else { 3 };
    let max_urls = if no_limit { usize::MAX } else { 200 };

    // Try to parse as sitemap index first (using safe parsing to handle malformed XML)
    if let Some(index) = safe_parse_sitemap_index(content) {
        if max_depth == 0 {
            return Err(anyhow!("Max sitemap depth reached"));
        }

        // Use the same prioritization logic as robots.txt processing
        let all_sitemaps: Vec<String> = index.sitemap.iter().map(|s| s.loc.clone()).collect();
        let prioritized = prioritize_sitemaps(all_sitemaps);

        // Fetch and parse sub-sitemaps
        for sitemap_url in prioritized.iter().take(max_sitemaps) {
            if let Ok(sub_content) = fetch_sitemap(client, sitemap_url).await {
                if let Ok(sub_urls) = Box::pin(extract_urls_from_sitemap_recursive(
                    client,
                    &sub_content,
                    domain,
                    max_depth - 1,
                    no_limit,
                ))
                .await
                {
                    urls.extend(sub_urls);
                    if urls.len() >= max_urls {
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
async fn fetch_robots_sitemaps(client: &CachingClient, domain: &str) -> Vec<String> {
    let robots_url = format!("https://{}/robots.txt", domain);

    let content = match client.fetch_html(&robots_url).await {
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
                line.get(8..).map(|s| s.trim().to_string())
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
    client: &CachingClient,
    domain: &str,
    urls_per_site: usize,
    min_year: u32,
    no_limit: bool,
) -> Result<Vec<String>> {
    let homepage_url = format!("https://{}/", domain);
    let html = client
        .fetch_html(&homepage_url)
        .await
        .map_err(|e| anyhow!("Failed to fetch homepage: {}", e))?;
    let document = Html::parse_document(&html);
    let link_selector = Selector::parse("a[href]").unwrap();

    let mut recipe_urls = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let limit = if no_limit { usize::MAX } else { urls_per_site };

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
                        && is_recipe_url(&full_url, min_year)
                        && seen.insert(full_url.clone())
                    {
                        recipe_urls.push(full_url);
                        if recipe_urls.len() >= limit {
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

/// Detect monthly/yearly archive pages with no recipe slug (e.g., /2026/01/)
fn is_archive_page(url: &str) -> bool {
    static ARCHIVE_PATTERN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"/\d{4}/\d{2}/?$").unwrap());

    ARCHIVE_PATTERN.is_match(url)
}

/// Detect roundup/collection pages that aggregate multiple recipes
fn is_roundup_url(url: &str) -> bool {
    // Strip .html suffix so patterns with $ anchors match correctly
    let url = url.strip_suffix(".html").unwrap_or(url);

    static ROUNDUP_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        vec![
            // "30-easy-recipes", "50-best-chicken-recipes" (plural only, not singular -recipe)
            Regex::new(r"/\d+(-[a-z]+)+-recipes(/|$)").unwrap(),
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
            // Multi-word slugs ending in -recipes, optionally followed by a number
            // e.g., "fall-soup-recipes", "game-day-recipes", "best-fall-dinner-recipes-2"
            Regex::new(r"/[a-z]+(-[a-z0-9]+)*-recipes(-\d+)?/?$").unwrap(),
            // "50-healthy-recipes-to-kick-off"
            Regex::new(r"/\d+(-[a-z]+)*-recipes-to-").unwrap(),
            // "25-recipes-that-should" - number followed by recipes
            Regex::new(r"/\d+-recipes-").unwrap(),
            // "12-summer-recipes-i-forgot", "20-best-shrimp-recipes-for-weeknight-dinners"
            Regex::new(r"/\d+(-[a-z]+)+-recipes-[a-z]").unwrap(),
            // "christmas-recipes-for-your-holiday-table", "soup-recipes-to-warm-you-up"
            Regex::new(r"/[a-z]+-recipes-[a-z]").unwrap(),
            // "recipes-for-january", "recipes-for-the-week"
            Regex::new(r"/recipes-for-[a-z]").unwrap(),
            // Spelled-out number roundups with collection words:
            // "six-spooky-cocktails-for-spirited", "four-quick-and-easy-easter-treats-for"
            // Requires a collection word (recipes/treats/cocktails/etc.) to avoid false positives
            // on recipe names like "five-spice-beef" or "three-bean-chili"
            Regex::new(r"/(three|four|five|six|seven|eight|nine|ten|twelve|fifteen|twenty|thirty|fifty)(-[a-z0-9]+)*-(recipes|treats|cocktails|desserts|meals|appetizers|snacks|ideas|drinks)").unwrap(),
            // "new-years-eve-recipes-and-ideas" - recipes-and-X is always a roundup
            Regex::new(r"-recipes-and-").unwrap(),
            // Collection adjective + plural food category at end of URL slug:
            // "summer-weeknight-meals", "best-back-to-school-dinners", "healthy-dinner-ideas",
            // "spring-cocktails-and-mocktails"
            Regex::new(r"/(best|summer|fall|winter|spring|healthy|easy|quick)(-[a-z]+)*-(meals|dinners|lunches|breakfasts|cocktails|mocktails|appetizers|snacks|ideas)/?$").unwrap(),
        ]
    });

    ROUNDUP_PATTERNS.iter().any(|re| re.is_match(url))
}

/// Detect non-recipe blog posts: giveaways, link roundups, how-to guides, etc.
fn is_non_recipe_post(url: &str) -> bool {
    static NON_RECIPE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        vec![
            // Giveaways
            Regex::new(r"[-/]giveaway").unwrap(),
            // Link roundups from other sites
            Regex::new(r"bites-from-other-blogs").unwrap(),
            Regex::new(r"links?-i-love").unwrap(),
            Regex::new(r"link-love").unwrap(),
            Regex::new(r"weekly-links").unwrap(),
            Regex::new(r"friday-links").unwrap(),
            // How-to guides that explicitly say "without a recipe"
            Regex::new(r"without-a-recipe").unwrap(),
            // Product reviews and announcements
            Regex::new(r"[-/]product-review").unwrap(),
            Regex::new(r"[-/]book-review").unwrap(),
            Regex::new(r"[-/]introducing-").unwrap(),
            Regex::new(r"cookbook-pre-order").unwrap(),
            // Travel and personal posts
            Regex::new(r"[-/]trip-to-").unwrap(),
            Regex::new(r"[-/]vacation-").unwrap(),
            Regex::new(r"[-/]field-trip").unwrap(),
            // Meta/personal blog posts
            Regex::new(r"[-/]i-started/?$").unwrap(),
            Regex::new(r"[-/]a-boys-job").unwrap(),
            Regex::new(r"[-/]my-happy-place").unwrap(),
            // Lifestyle series posts
            Regex::new(r"a-week-in-the-life").unwrap(),
            Regex::new(r"currently-crushing-on").unwrap(),
            Regex::new(r"tuesday-things").unwrap(),
            Regex::new(r"what-to-eat-this-week").unwrap(),
            // Generic tips without recipes
            Regex::new(r"[-/]tips-for-").unwrap(),
            Regex::new(r"[-/]gift-guide").unwrap(),
            Regex::new(r"[-/]holiday-gift").unwrap(),
            Regex::new(r"[-/]gifts-for-").unwrap(),
            // Year-in-review and best-of posts
            Regex::new(r"year-in-review").unwrap(),
            Regex::new(r"best-of-\d{4}").unwrap(),
            // Equipment/decor/non-food posts
            Regex::new(r"[-/]kitchen-essentials").unwrap(),
            Regex::new(r"[-/]holiday-tablescape").unwrap(),
            Regex::new(r"[-/]inspiring-instagrammers").unwrap(),
        ]
    });

    NON_RECIPE_PATTERNS.iter().any(|re| re.is_match(url))
}

/// Check if a /recipes/SLUG slug looks like a category page rather than an individual recipe.
/// Category slugs are short 1-2 word phrases naming a food category or site section,
/// e.g. "healthy-choices", "weeknight-meals", "indian-breakfast", "latest-updates".
fn is_category_slug(slug: &str) -> bool {
    static CATEGORY_WORDS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
        vec![
            // Food categories (plural = collection, not individual recipe)
            "meals",
            "dinners",
            "lunches",
            "breakfasts",
            "desserts",
            "snacks",
            "appetizers",
            "cocktails",
            "mocktails",
            "salads",
            "soups",
            "sides",
            // Site navigation / meta sections
            "latest-updates",
            "most-popular",
            "quick-and-easy",
            "healthy-choices",
        ]
    });

    // Check if the slug ends with a category word
    // This catches "indian-breakfast" (ends with "breakfast" — but that's singular, skip)
    // and "weeknight-meals" (ends with "meals"), "sweets-desserts" (ends with "desserts")
    for word in CATEGORY_WORDS.iter() {
        if slug == *word || slug.ends_with(&format!("-{}", word)) {
            return true;
        }
    }

    // Also check for singular food category words, but only for short slugs (1-2 words).
    // Long slugs like "sausage-and-potatoes-sheet-pan-dinner" are real recipes, not categories.
    let hyphen_count = slug.matches('-').count();
    if hyphen_count <= 1 {
        static SINGULAR_CATEGORIES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
            vec![
                "breakfast",
                "dinner",
                "lunch",
                "dessert",
                "snack",
                "appetizer",
            ]
        });
        for word in SINGULAR_CATEGORIES.iter() {
            if slug == *word || slug.ends_with(&format!("-{}", word)) {
                return true;
            }
        }
    }

    false
}

/// Filter out old posts that predate reliable structured recipe data
/// Check for dated blog post pattern, returning the year if found
/// Returns Some(year) if URL has /YYYY/MM/ pattern, None otherwise
fn extract_post_year(url: &str) -> Option<u32> {
    static DATE_PATTERN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"/(\d{4})/\d{2}/").unwrap());

    DATE_PATTERN
        .captures(url)
        .and_then(|caps| caps.get(1))
        .and_then(|year_str| year_str.as_str().parse::<u32>().ok())
}

fn is_recipe_url(url: &str, min_year: u32) -> bool {
    // Strip query strings and fragment identifiers before analysis
    let url_clean = url.split('?').next().unwrap_or(url);
    let url_clean = url_clean.split('#').next().unwrap_or(url_clean);
    let lower = url_clean.to_lowercase();

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

    if lower.contains("kingarthurbaking.com") {
        // KAB recipes are always under /recipes/SLUG where slug ends with -recipe
        // Blog posts (/blog/*) are articles/tips, not recipe pages
        // Category pages (/recipes/muffins-popovers) lack -recipe suffix
        return lower.contains("/recipes/") && lower.contains("-recipe");
    }

    if lower.contains("tasty.co") {
        // Compilations are collection pages, not individual recipes
        if lower.contains("/compilation/") {
            return false;
        }
    }

    if lower.contains("food.com") {
        // /recipe/all/* are category listing pages (popular, trending)
        if lower.contains("/recipe/all/") {
            return false;
        }
    }

    if lower.contains("foodnetwork.com") {
        // /fn-dish/* are article pages, not recipe pages
        if lower.contains("/fn-dish/") {
            return false;
        }
    }

    if lower.contains("feastingathome.com") {
        // /recipe-cuisine/* are category pages listing recipes by cuisine
        if lower.contains("/recipe-cuisine/") {
            return false;
        }
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

    // Monthly/yearly archive pages with no slug (e.g., /2026/01/)
    if is_archive_page(&lower) {
        return false;
    }

    // Roundup/collection patterns
    if is_roundup_url(&lower) {
        return false;
    }

    // Non-recipe blog posts (giveaways, link roundups, how-to guides, etc.)
    if is_non_recipe_post(&lower) {
        return false;
    }

    // Check for dated blog post - reject old posts, remember for later acceptance
    let post_year = extract_post_year(&lower);
    if let Some(year) = post_year {
        if year < min_year {
            return false;
        }
    }

    // === PHASE 3: POSITIVE INDICATORS ===
    // Check for patterns that suggest individual recipes

    // Strong positive: singular "/recipe/" in path
    if lower.contains("/recipe/") {
        return true;
    }

    // Check "/recipes/" followed by a slug (not ending there)
    if let Some((_, after_recipes)) = lower.split_once("/recipes/") {
        // Strip trailing slash if present
        let slug = after_recipes.trim_end_matches('/');
        // Must have content after /recipes/ and not be a category path or nested
        // Require either: hyphen + long enough (>12 chars) OR very long (>25 chars)
        // This filters category pages like /recipes/dinner, /recipes/hearty-meals
        // while allowing recipes like /recipes/minestrone-soup, /recipes/beef-carpaccio
        if !slug.is_empty()
            && !slug.starts_with("category")
            && !slug.contains('/')
            && ((slug.contains('-') && slug.len() > 12) || slug.len() > 25)
            && !is_category_slug(slug)
        {
            return true;
        }
    }

    // "-recipe" or "-recipe/" suffix (singular, specific recipe)
    // But NOT "-recipes" (plural = collection)
    if (lower.contains("-recipe/") || lower.ends_with("-recipe")) && !lower.contains("-recipes") {
        return true;
    }

    // Date-based blog URLs from min_year+ (already passed old post filter)
    if post_year.is_some() {
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
        assert!(is_recipe_url(
            "https://example.com/recipe/chocolate-cake",
            2016
        ));
        assert!(is_recipe_url(
            "https://example.com/recipes/minestrone-soup",
            2016
        ));
        assert!(is_recipe_url(
            "https://example.com/chocolate-cake-recipe/",
            2016
        ));
        assert!(is_recipe_url(
            "https://example.com/2025/01/my-delicious-recipe",
            2016
        ));

        // Negative cases - static pages
        assert!(!is_recipe_url("https://example.com/about", 2016));
        assert!(!is_recipe_url(
            "https://example.com/category/desserts",
            2016
        ));
        assert!(!is_recipe_url("https://example.com/tag/chocolate", 2016));
    }

    #[test]
    fn test_roundup_detection() {
        // Roundup patterns should be rejected
        assert!(!is_recipe_url("https://example.com/30-easy-recipes/", 2016));
        assert!(!is_recipe_url(
            "https://example.com/top-10-best-soup-recipes/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://example.com/christmas-recipe-roundup/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://example.com/favorite-recipes-of-2021/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://twopeasandtheirpod.com/our-10-favorite-recipes-from-2011/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://example.com/50-healthy-recipes-to-kick-off-2012/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://example.com/healthy-dinner-recipes/",
            2016
        ));
        assert!(!is_recipe_url("https://example.com/all-recipes/", 2016));

        // But individual recipes with numbers should be accepted
        assert!(is_recipe_url(
            "https://example.com/5-ingredient-pasta-recipe/",
            2016
        ));
        assert!(is_recipe_url(
            "https://example.com/20-minute-chicken-recipe/",
            2016
        ));
    }

    #[test]
    fn test_category_exclusion() {
        // Category pages should be rejected
        assert!(!is_recipe_url(
            "https://davidlebovitz.com/category/recipes/breads/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://example.com/category/recipes/desserts/",
            2016
        ));

        // Malformed paths should be rejected
        assert!(!is_recipe_url(
            "https://smittenkitchen.com/./recipes/vegetable/cabbage/",
            2016
        ));

        // Fragment URLs: fragment is stripped, so the base URL is evaluated
        // The base URL /2025/01/cake/ is a valid date-based recipe post
        assert!(is_recipe_url(
            "https://example.com/2025/01/cake/#comments",
            2016
        ));

        // Category index pages (/recipes/X with short single/two-word slug) should be rejected
        assert!(!is_recipe_url(
            "https://pinchofyum.com/recipes/dinner",
            2016
        ));
        assert!(!is_recipe_url("https://pinchofyum.com/recipes/pasta", 2016));
        assert!(!is_recipe_url(
            "https://pinchofyum.com/recipes/casserole/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://minimalistbaker.com/recipes/hearty-meals/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://minimalistbaker.com/recipes/sweet-things/",
            2016
        ));

        // But actual recipe slugs with 3+ words should be accepted
        assert!(is_recipe_url(
            "https://example.com/recipes/chocolate-chip-cookies/",
            2016
        ));
        assert!(is_recipe_url(
            "https://example.com/recipes/beef-brisket-pot-roast/",
            2016
        ));
    }

    #[test]
    fn test_non_recipe_posts() {
        // Giveaways should be rejected
        assert!(!is_recipe_url(
            "https://bakingbites.com/2015/03/baking-bites-easter-giveaway/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://example.com/2020/01/kitchen-giveaway/",
            2016
        ));

        // Link roundups should be rejected
        assert!(!is_recipe_url(
            "https://bakingbites.com/2015/01/bites-from-other-blogs-125/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://example.com/2020/01/links-i-love/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://example.com/2020/01/friday-links/",
            2016
        ));

        // Travel/personal posts should be rejected (old posts without recipe signal)
        // Note: 2010 posts are rejected as too old, and field-trip matches the exclusion pattern
        assert!(!is_recipe_url(
            "https://bakingbites.com/2010/04/baking-bites-in-kauai/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://example.com/2020/01/field-trip/",
            2016
        ));

        // Gift guides should be rejected
        assert!(!is_recipe_url(
            "https://loveandoliveoil.com/2022/11/ultimate-foodie-holiday-gift-guide/",
            2016
        ));
    }

    #[test]
    fn test_old_post_filtering() {
        // Posts before 2016 should be rejected with default min_year
        assert!(!is_recipe_url(
            "https://bakingbites.com/2004/12/chocolate-cake/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://howsweeteats.com/2009/09/random-post/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://example.com/2010/01/old-recipe/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://example.com/2012/01/chocolate-cake/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://example.com/2015/01/chocolate-cake/",
            2016
        ));

        // 2016+ posts should be accepted
        assert!(is_recipe_url(
            "https://example.com/2016/01/chocolate-cake/",
            2016
        ));
        assert!(is_recipe_url(
            "https://example.com/2023/01/chocolate-cake/",
            2016
        ));
        assert!(is_recipe_url(
            "https://alexandracooks.com/2019/06/21/fish-en-papillote/",
            2016
        ));

        // With lower min_year, old posts should be accepted
        assert!(is_recipe_url(
            "https://example.com/2010/01/old-recipe/",
            2006
        ));
        assert!(is_recipe_url(
            "https://smittenkitchen.com/2006/08/moules-frites/",
            2006
        ));
        assert!(!is_recipe_url(
            "https://example.com/2005/01/ancient-post/",
            2006
        ));
    }

    #[test]
    fn test_valid_recipe_url_patterns() {
        // /recipe/ path pattern
        assert!(is_recipe_url(
            "https://altonbrown.com/recipes/beef-carpaccio/",
            2016
        ));
        assert!(is_recipe_url(
            "https://slenderkitchen.com/recipe/salmon-burgers",
            2016
        ));

        // -recipe suffix pattern
        assert!(is_recipe_url(
            "https://therecipecritic.com/avocado-toast-recipe/",
            2016
        ));
        assert!(is_recipe_url(
            "https://cookingclassy.com/brownie-recipe/",
            2016
        ));
    }

    #[test]
    fn test_seriouseats_recipe_url_filtering() {
        // Individual recipes should be accepted (singular "-recipe-" pattern)
        assert!(is_recipe_url(
            "https://www.seriouseats.com/hot-milk-cake-recipe-11878680",
            2016
        ));
        assert!(is_recipe_url(
            "https://www.seriouseats.com/lentil-sausage-stew-recipe-11880797",
            2016
        ));
        assert!(is_recipe_url(
            "https://www.seriouseats.com/binakol-na-manok-filipino-chicken-and-coconut-soup-recipe-11878784",
            2016
        ));

        // Collection pages should be rejected (plural "-recipes-" pattern)
        assert!(!is_recipe_url(
            "https://www.seriouseats.com/vegan-dinner-recipes-11878691",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.seriouseats.com/tahini-recipes-beyond-hummus-11878978",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.seriouseats.com/most-saved-shrimp-recipes-11879657",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.seriouseats.com/hearty-chickpea-recipes-11878318",
            2016
        ));
    }

    #[test]
    fn test_category_slug_filtering() {
        // Category pages under /recipes/ should be rejected
        assert!(!is_recipe_url(
            "https://pinchofyum.com/recipes/healthy-choices",
            2016,
        ));
        assert!(!is_recipe_url(
            "https://pinchofyum.com/recipes/quick-and-easy",
            2016,
        ));
        assert!(!is_recipe_url(
            "https://www.howsweeteats.com/recipes/weeknight-meals/",
            2016,
        ));
        assert!(!is_recipe_url(
            "https://www.indianhealthyrecipes.com/recipes/indian-breakfast/",
            2016,
        ));
        assert!(!is_recipe_url(
            "https://www.indianhealthyrecipes.com/recipes/latest-updates/",
            2016,
        ));
        assert!(!is_recipe_url(
            "https://www.indianhealthyrecipes.com/recipes/sweets-desserts/",
            2016,
        ));

        // But real recipe slugs should still be accepted
        assert!(is_recipe_url(
            "https://example.com/recipes/minestrone-soup",
            2016,
        ));
        assert!(is_recipe_url(
            "https://example.com/recipes/chocolate-chip-cookies/",
            2016,
        ));
        assert!(is_recipe_url(
            "https://example.com/recipes/grilled-chicken-salad/",
            2016,
        ));
        // Long slugs ending in food words are real recipes, not categories
        assert!(is_recipe_url(
            "https://www.jocooks.com/recipes/sausage-and-potatoes-sheet-pan-dinner/",
            2016,
        ));
        assert!(is_recipe_url(
            "https://www.jocooks.com/recipes/ranch-pork-chops-potatoes-sheet-pan-dinner/",
            2016,
        ));
    }

    #[test]
    fn test_collection_roundup_patterns() {
        // "recipes-and-X" is always a roundup
        assert!(!is_recipe_url(
            "https://damndelicious.net/2025/12/28/new-years-eve-recipes-and-ideas/",
            2016,
        ));

        // Collection adjective + plural food category
        assert!(!is_recipe_url(
            "https://www.howsweeteats.com/2026/01/healthy-dinner-ideas/",
            2016,
        ));
        assert!(!is_recipe_url(
            "https://www.howsweeteats.com/2025/03/spring-cocktails-and-mocktails/",
            2016,
        ));
        assert!(!is_recipe_url(
            "https://www.howsweeteats.com/2025/07/summer-weeknight-meals/",
            2016,
        ));
        assert!(!is_recipe_url(
            "https://www.howsweeteats.com/2025/08/best-back-to-school-dinners/",
            2016,
        ));

        // But real recipes that happen to end with food words should still pass
        assert!(is_recipe_url(
            "https://example.com/2025/01/crispy-chicken-dinner/",
            2016,
        ));
        assert!(is_recipe_url(
            "https://example.com/2025/01/chocolate-lava-cake/",
            2016,
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

    #[test]
    fn test_query_string_and_fragment_stripping() {
        // Query strings are stripped — filtered listings become bare /recipes/ path (rejected)
        assert!(!is_recipe_url(
            "https://food.com/recipe/all/popular?ref=nav",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.halfbakedharvest.com/recipes/?_recipe_meal=bread-recipes",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.halfbakedharvest.com/recipes/?_recipe_search=Dip&_recipe_meal=appetizers",
            2016
        ));

        // Fragments are stripped — base URL is evaluated
        assert!(!is_recipe_url(
            "https://food.com/recipe/all/trending#questions",
            2016
        ));

        // Valid recipe URLs with query strings still pass after stripping
        assert!(is_recipe_url(
            "https://www.kingarthurbaking.com/recipes/flaky-puff-crust-pizza-recipe?from=search-overlay",
            2016
        ));
        assert!(is_recipe_url(
            "https://food.com/recipe/breaded-eggplant-oven-baked-160089#questions",
            2016
        ));
    }

    #[test]
    fn test_kingarthurbaking_site_specific() {
        // Blog posts should be rejected
        assert!(!is_recipe_url(
            "https://www.kingarthurbaking.com/blog/2021/05/24/prebake-pie-crust",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.kingarthurbaking.com/blog/2022/07/14/8-reasons-your-cakes-turn-out-dry",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.kingarthurbaking.com/blog/2026/01/02/recipe-of-the-year-flaky-pizza",
            2016
        ));

        // Category pages should be rejected (no -recipe suffix)
        assert!(!is_recipe_url(
            "https://kingarthurbaking.com/recipes/muffins-popovers",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.kingarthurbaking.com/recipes/pasta-noodles",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.kingarthurbaking.com/recipes/biscuits-shortcakes",
            2016
        ));

        // Actual recipes should pass (slug ends with -recipe)
        assert!(is_recipe_url(
            "https://www.kingarthurbaking.com/recipes/flaky-puff-crust-pizza-recipe",
            2016
        ));
        assert!(is_recipe_url(
            "https://kingarthurbaking.com/recipes/no-bake-chocolate-and-date-energy-bars-recipe",
            2016
        ));
    }

    #[test]
    fn test_tasty_compilation_rejection() {
        assert!(!is_recipe_url(
            "https://tasty.co/compilation/5-best-chicken-wings-recipe",
            2016
        ));
        assert!(!is_recipe_url(
            "https://tasty.co/compilation/easy-and-delicious-spicy-appetizers-recipe",
            2016
        ));
        // But actual tasty.co recipes should pass
        assert!(is_recipe_url(
            "https://tasty.co/recipe/easy-butter-chicken",
            2016
        ));
    }

    #[test]
    fn test_food_com_category_rejection() {
        assert!(!is_recipe_url("https://food.com/recipe/all/trending", 2016));
        assert!(!is_recipe_url(
            "https://food.com/recipe/all/popular?ref=nav",
            2016
        ));
        // But actual food.com recipes should pass
        assert!(is_recipe_url(
            "https://food.com/recipe/tender-pot-roast-22137",
            2016
        ));
    }

    #[test]
    fn test_foodnetwork_fn_dish_rejection() {
        assert!(!is_recipe_url(
            "https://www.foodnetwork.com/fn-dish/recipes/food-networks-healthy-recipe-tricks",
            2016
        ));
    }

    #[test]
    fn test_feastingathome_category_rejection() {
        assert!(!is_recipe_url(
            "https://www.feastingathome.com/recipe-cuisine/asian-recipe/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.feastingathome.com/recipe-cuisine/chili-recipe/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.feastingathome.com/recipe-cuisine/stir-fry-recipe/",
            2016
        ));
    }

    #[test]
    fn test_archive_page_rejection() {
        // Monthly archive with no slug should be rejected
        assert!(!is_recipe_url(
            "https://www.halfbakedharvest.com/2026/01/",
            2016
        ));
        // But a post within the same month should pass
        assert!(is_recipe_url(
            "https://www.halfbakedharvest.com/2026/01/easy-chicken-soup/",
            2016
        ));
    }

    #[test]
    fn test_improved_roundup_patterns() {
        // Multi-word slugs ending in -recipes
        assert!(!is_recipe_url(
            "https://www.howsweeteats.com/2025/10/fall-soup-recipes/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.howsweeteats.com/2026/01/game-day-recipes/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.howsweeteats.com/2026/01/blood-orange-recipes/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.howsweeteats.com/2025/09/best-fall-dinner-recipes-2/",
            2016
        ));

        // Word-recipes-word patterns
        assert!(!is_recipe_url(
            "https://damndelicious.net/2025/12/16/christmas-recipes-for-your-holiday-table/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://damndelicious.net/2026/01/27/winning-super-bowl-recipes/",
            2016
        ));

        // onceuponachef roundups with .html
        assert!(!is_recipe_url(
            "https://www.onceuponachef.com/recipes/50-best-chicken-recipes.html",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.onceuponachef.com/recipes/20-best-shrimp-recipes-for-weeknight-dinners.html",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.onceuponachef.com/recipes/st-patricks-day-recipes.html",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.onceuponachef.com/recipes/soup-recipes-to-warm-you-up.html",
            2016
        ));

        // Spelled-out number roundups
        assert!(!is_recipe_url(
            "https://www.sprinklebakes.com/2018/03/four-quick-and-easy-easter-treats-for.html",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.sprinklebakes.com/2020/10/six-spooky-cocktails-for-spirited.html",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.sprinklebakes.com/2020/10/six-spooky-treats-for-sweet-halloween.html",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.sprinklebakes.com/2018/12/three-classic-christmas-treats-to-make.html",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.sprinklebakes.com/2019/12/20-quick-and-easy-holiday-candy-recipes.html",
            2016
        ));

        // recipes-for- pattern
        assert!(!is_recipe_url(
            "https://www.howsweeteats.com/2026/01/recipes-for-january/",
            2016
        ));

        // But individual recipes with numbers should still be accepted
        assert!(is_recipe_url(
            "https://example.com/5-ingredient-pasta-recipe/",
            2016
        ));
    }

    #[test]
    fn test_improved_non_recipe_posts() {
        // Lifestyle series
        assert!(!is_recipe_url(
            "https://www.howsweeteats.com/2026/01/a-week-in-the-life-vol-1-7/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.howsweeteats.com/2026/01/currently-crushing-on-608/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.howsweeteats.com/2026/01/tuesday-things-753/",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.howsweeteats.com/2026/01/what-to-eat-this-week-1-25-26/",
            2016
        ));

        // Product/promo posts
        assert!(!is_recipe_url(
            "https://www.loveandoliveoil.com/2025/07/introducing-fresh-baked-puns.html",
            2016
        ));
        assert!(!is_recipe_url(
            "https://bakingbites.com/2018/04/perfectly-creamy-frozen-yogurt-cookbook-pre-order/",
            2016
        ));

        // Year-in-review
        assert!(!is_recipe_url(
            "https://www.sprinklebakes.com/2016/12/sprinkle-bakes-2016-year-in-review.html",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.loveandoliveoil.com/2025/12/the-best-of-2025.html",
            2016
        ));

        // Gift/tablescape/equipment posts
        assert!(!is_recipe_url(
            "https://www.sprinklebakes.com/2018/11/gifts-for-baker-on-your-list-2.html",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.sprinklebakes.com/2018/11/a-holiday-tablescape-with-tartan-and.html",
            2016
        ));
        assert!(!is_recipe_url(
            "https://www.loveandoliveoil.com/2017/09/kitchen-essentials.html",
            2016
        ));

        // Inspiring instagrammers
        assert!(!is_recipe_url(
            "https://www.101cookbooks.com/13-inspiring-instagrammers-to-follow-for-healthy-feelgood-food-recipe/",
            2016
        ));
    }
}
