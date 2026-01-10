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
    // Try standard sitemap.xml location
    let sitemap_url = format!("https://{}/sitemap.xml", domain);
    let mut all_urls = Vec::new();

    let sitemap_content = match fetch_sitemap(client, &sitemap_url).await {
        Ok(content) => content,
        Err(_) => {
            // Try robots.txt for sitemap location
            if let Some(sitemap_from_robots) = try_robots_txt(client, domain).await {
                fetch_sitemap(client, &sitemap_from_robots).await?
            } else {
                return Err(anyhow!("No sitemap found"));
            }
        }
    };

    // Parse the sitemap (handles both urlset and sitemapindex)
    all_urls
        .extend(extract_urls_from_sitemap_recursive(client, &sitemap_content, domain, 2).await?);

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

/// Recursively extract URLs from a sitemap, following sitemap indexes up to max_depth levels
async fn extract_urls_from_sitemap_recursive(
    client: &reqwest::Client,
    content: &str,
    domain: &str,
    max_depth: usize,
) -> Result<Vec<String>> {
    let mut urls = Vec::new();

    // Try to parse as sitemap index first
    if let Ok(index) = from_str::<SitemapIndex>(content) {
        if max_depth == 0 {
            return Err(anyhow!("Max sitemap depth reached"));
        }

        // Prioritize recipe-specific sitemaps
        let mut sitemaps_to_fetch: Vec<&str> = index
            .sitemap
            .iter()
            .filter(|s| {
                let loc_lower = s.loc.to_lowercase();
                loc_lower.contains("recipe") || loc_lower.contains("post")
            })
            .map(|s| s.loc.as_str())
            .collect();

        // If no recipe-specific sitemaps, try all of them (but limit to first 5)
        if sitemaps_to_fetch.is_empty() {
            sitemaps_to_fetch = index
                .sitemap
                .iter()
                .take(5)
                .map(|s| s.loc.as_str())
                .collect();
        }

        // Fetch and parse sub-sitemaps
        for sitemap_url in sitemaps_to_fetch.into_iter().take(3) {
            // Limit to 3 to avoid too many requests
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

    // Try to parse as urlset
    if let Ok(urlset) = from_str::<Urlset>(content) {
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

async fn try_robots_txt(client: &reqwest::Client, domain: &str) -> Option<String> {
    let robots_url = format!("https://{}/robots.txt", domain);

    let response = client.get(&robots_url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    let content = response.text().await.ok()?;

    // Look for Sitemap: directive
    for line in content.lines() {
        let line = line.trim();
        if line.to_lowercase().starts_with("sitemap:") {
            let sitemap_url = line[8..].trim();
            return Some(sitemap_url.to_string());
        }
    }

    None
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

fn is_recipe_url(url: &str) -> bool {
    let lower = url.to_lowercase();

    // SeriousEats-specific: reject collection pages (plural "recipes" pattern)
    // Individual recipes use singular "-recipe-" followed by ID
    // Collections use plural "-recipes-" followed by ID
    if lower.contains("seriouseats.com") {
        // Reject if it has the collection pattern (plural "-recipes-")
        if lower.contains("-recipes-") {
            return false;
        }
        // For seriouseats, require the singular "-recipe-" pattern
        return lower.contains("-recipe-");
    }

    // Common recipe URL patterns
    lower.contains("/recipe/")
        || lower.contains("/recipes/")
        || lower.contains("-recipe")
        || lower.contains("/20")  // Date-based URLs common in blogs (e.g., /2025/01/)
        && !lower.contains("/category/")
        && !lower.contains("/tag/")
        && !lower.contains("/page/")
        && !lower.contains("/author/")
        && !lower.contains("/about")
        && !lower.contains("/contact")
        && !lower.contains("/privacy")
        && !lower.contains("/terms")
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
    fn test_is_recipe_url() {
        assert!(is_recipe_url("https://example.com/recipe/chocolate-cake"));
        assert!(is_recipe_url("https://example.com/recipes/soup"));
        assert!(is_recipe_url("https://example.com/chocolate-cake-recipe"));
        assert!(is_recipe_url("https://example.com/2025/01/my-recipe"));

        assert!(!is_recipe_url("https://example.com/about"));
        assert!(!is_recipe_url("https://example.com/category/desserts"));
        assert!(!is_recipe_url("https://example.com/tag/chocolate"));
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
}
