use std::sync::LazyLock;

use regex::Regex;

use crate::error::ExtractError;
use crate::types::{ExtractRecipeOutput, ExtractionAttempt, ExtractionMethod, RawRecipe};
use scraper::{Html, Selector};

/// Regex to find JSON-LD script tags (case-insensitive for type attribute)
static JSONLD_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<script[^>]*type\s*=\s*["']application/ld\+json["'][^>]*>(.*?)</script>"#)
        .expect("Invalid JSON-LD regex")
});

/// Regex to find og:image meta tag
static OG_IMAGE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<meta[^>]*property\s*=\s*["']og:image["'][^>]*content\s*=\s*["']([^"']+)["'][^>]*/?\s*>"#)
        .expect("Invalid og:image regex")
});

/// Alternative og:image regex (content before property)
static OG_IMAGE_REGEX_ALT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<meta[^>]*content\s*=\s*["']([^"']+)["'][^>]*property\s*=\s*["']og:image["'][^>]*/?\s*>"#)
        .expect("Invalid og:image alt regex")
});

/// Regex to validate that text after a letter→digit boundary looks like a new
/// ingredient quantity. Matches a digit followed by a space, fraction slash,
/// or metric/imperial unit.
static INGREDIENT_START_AFTER_SPLIT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\d+(\s|/\d|g\b|kg\b|mg\b|ml\b|l\b|oz|lb|cup|teaspoon|tablespoon|tsp\b|tbsp\b|pound|ounce)")
        .expect("Invalid ingredient start after split regex")
});

/// Extract a recipe from HTML containing JSON-LD structured data.
/// Falls back to microdata extraction if JSON-LD fails, then tries
/// supplementing partial structured data with HTML class-based extraction.
///
/// Uses a fast regex-based path for JSON-LD to avoid full DOM parsing.
pub fn extract_recipe(html: &str, source_url: &str) -> Result<RawRecipe, ExtractError> {
    // Fast path: extract JSON-LD using regex (avoids DOM parsing)
    if let Some(mut recipe) = extract_jsonld_fast(html, source_url) {
        // Structured data provides a flat ingredient list; group headers
        // (e.g. "Meatballs", "Broth") only exist in the HTML.
        // Only parse the DOM when a group marker is present.
        if html.contains("wprm-recipe-group-name") || html.contains("jetpack-recipe-ingredients") {
            let document = Html::parse_document(html);
            supplement_ingredient_groups(&mut recipe, &document);
        }
        return Ok(recipe);
    }

    // Slow path: full DOM parsing for malformed HTML or microdata-only sites
    let document = Html::parse_document(html);

    // Try JSON-LD via DOM (handles edge cases regex might miss)
    if let Ok(mut recipe) = extract_recipe_from_jsonld(&document, source_url) {
        supplement_ingredient_groups(&mut recipe, &document);
        return Ok(recipe);
    }

    // Fall back to microdata
    if let Ok(mut recipe) = extract_recipe_from_microdata(&document, source_url) {
        supplement_ingredient_groups(&mut recipe, &document);
        return Ok(recipe);
    }

    // Last resort: supplement partial structured data with HTML fallbacks
    extract_recipe_with_html_fallback(html, &document, source_url)
}

/// Try to replace flat ingredients with a grouped version from HTML.
/// Supports WPRM and Jetpack recipe plugins.
fn supplement_ingredient_groups(recipe: &mut RawRecipe, document: &Html) {
    if let Some(grouped) = extract_wprm_ingredients_with_groups(document)
        .or_else(|| extract_jetpack_ingredients_with_groups(document))
    {
        recipe.ingredients = grouped;
    }
}

/// Fast JSON-LD extraction using regex to avoid DOM parsing.
/// Returns None if no valid JSON-LD recipe is found.
fn extract_jsonld_fast(html: &str, source_url: &str) -> Option<RawRecipe> {
    for cap in JSONLD_REGEX.captures_iter(html) {
        let json_text = match cap.get(1) {
            Some(m) => m.as_str(),
            None => continue,
        };

        // Sanitize and parse JSON
        let sanitized = sanitize_json(json_text);
        let json: serde_json::Value = match serde_json::from_str(&sanitized) {
            Ok(v) => v,
            Err(_) => continue, // Try next script tag
        };

        // Look for Recipe type
        if let Some(recipe) = find_recipe_in_json(&json) {
            if let Ok(mut raw_recipe) = extract_recipe_data(recipe, source_url) {
                // Fallback to og:image if no images found
                if raw_recipe.image_urls.is_empty() {
                    if let Some(og_image) = extract_og_image_fast(html) {
                        raw_recipe.image_urls.push(og_image);
                    }
                }
                // Extract footnotes from HTML when ingredients contain asterisks
                if raw_recipe.ingredients.contains('*') {
                    raw_recipe.footnotes = extract_footnotes_from_html(html);
                }
                return Some(raw_recipe);
            }
        }
    }
    None
}

/// Fast og:image extraction using regex.
fn extract_og_image_fast(html: &str) -> Option<String> {
    // Try property-first pattern
    if let Some(cap) = OG_IMAGE_REGEX.captures(html) {
        return cap.get(1).map(|m| m.as_str().to_string());
    }
    // Try content-first pattern
    if let Some(cap) = OG_IMAGE_REGEX_ALT.captures(html) {
        return cap.get(1).map(|m| m.as_str().to_string());
    }
    None
}

/// Extract a recipe, trying all methods and reporting which ones work.
/// Returns the first successful recipe along with stats for all methods tried.
///
/// Uses fast regex-based JSON-LD extraction when possible to avoid DOM parsing.
pub fn extract_recipe_with_stats(
    html: &str,
    source_url: &str,
) -> Result<ExtractRecipeOutput, ExtractError> {
    // Fast path: try regex-based JSON-LD extraction (avoids DOM parsing)
    if let Some(mut recipe) = extract_jsonld_fast(html, source_url) {
        if html.contains("wprm-recipe-group-name") || html.contains("jetpack-recipe-ingredients") {
            let document = Html::parse_document(html);
            supplement_ingredient_groups(&mut recipe, &document);
        }
        return Ok(ExtractRecipeOutput {
            raw_recipe: recipe,
            method_used: ExtractionMethod::JsonLd,
            all_attempts: vec![ExtractionAttempt {
                method: ExtractionMethod::JsonLd,
                success: true,
                error: None,
            }],
        });
    }

    // Slow path: full DOM parsing for malformed HTML or microdata-only sites
    let document = Html::parse_document(html);

    // Try JSON-LD via DOM (handles edge cases regex might miss)
    let jsonld_result = extract_recipe_from_jsonld(&document, source_url);
    if let Ok(mut recipe) = jsonld_result {
        supplement_ingredient_groups(&mut recipe, &document);
        return Ok(ExtractRecipeOutput {
            raw_recipe: recipe,
            method_used: ExtractionMethod::JsonLd,
            all_attempts: vec![ExtractionAttempt {
                method: ExtractionMethod::JsonLd,
                success: true,
                error: None,
            }],
        });
    }

    // Fall back to microdata
    let microdata_result = extract_recipe_from_microdata(&document, source_url);
    if let Ok(mut recipe) = microdata_result {
        supplement_ingredient_groups(&mut recipe, &document);
        return Ok(ExtractRecipeOutput {
            raw_recipe: recipe,
            method_used: ExtractionMethod::Microdata,
            all_attempts: vec![
                ExtractionAttempt {
                    method: ExtractionMethod::JsonLd,
                    success: false,
                    error: jsonld_result.as_ref().err().map(|e| e.to_string()),
                },
                ExtractionAttempt {
                    method: ExtractionMethod::Microdata,
                    success: true,
                    error: None,
                },
            ],
        });
    }

    // Last resort: supplement partial structured data with HTML fallbacks
    let html_fallback_result = extract_recipe_with_html_fallback(html, &document, source_url);
    match html_fallback_result {
        Ok(recipe) => Ok(ExtractRecipeOutput {
            raw_recipe: recipe,
            method_used: ExtractionMethod::HtmlFallback,
            all_attempts: vec![
                ExtractionAttempt {
                    method: ExtractionMethod::JsonLd,
                    success: false,
                    error: jsonld_result.as_ref().err().map(|e| e.to_string()),
                },
                ExtractionAttempt {
                    method: ExtractionMethod::Microdata,
                    success: false,
                    error: microdata_result.as_ref().err().map(|e| e.to_string()),
                },
                ExtractionAttempt {
                    method: ExtractionMethod::HtmlFallback,
                    success: true,
                    error: None,
                },
            ],
        }),
        Err(e) => Err(e),
    }
}

/// Extract recipe from JSON-LD script tags.
fn extract_recipe_from_jsonld(
    document: &Html,
    source_url: &str,
) -> Result<RawRecipe, ExtractError> {
    let selector = Selector::parse("script[type='application/ld+json']").expect("Invalid selector");

    for element in document.select(&selector) {
        let json_text = element.inner_html();

        // Sanitize JSON to handle malformed content (e.g., unescaped newlines)
        let sanitized = sanitize_json(&json_text);

        // Try to parse as JSON
        let json: serde_json::Value = match serde_json::from_str(&sanitized) {
            Ok(v) => v,
            Err(_) => continue, // Try next script tag
        };

        // Look for Recipe type
        if let Some(recipe) = find_recipe_in_json(&json) {
            let mut raw_recipe = extract_recipe_data(recipe, source_url)?;
            // Fallback to og:image if no images found in JSON-LD structured data
            if raw_recipe.image_urls.is_empty() {
                if let Some(og_image) = extract_og_image(document) {
                    raw_recipe.image_urls.push(og_image);
                }
            }
            // Extract footnotes when ingredients contain asterisks
            if raw_recipe.ingredients.contains('*') {
                raw_recipe.footnotes = extract_footnotes_from_document(document);
            }
            return Ok(raw_recipe);
        }
    }

    Err(ExtractError::NoRecipe)
}

/// Sanitize JSON-LD content to handle common malformed patterns.
/// Some sites include literal newlines/tabs inside JSON strings instead of escaped versions.
fn sanitize_json(json: &str) -> String {
    let mut result = String::with_capacity(json.len());
    let mut in_string = false;
    let mut prev_char = '\0';

    for c in json.chars() {
        if c == '"' && prev_char != '\\' {
            in_string = !in_string;
            result.push(c);
        } else if in_string {
            // Escape control characters inside strings
            match c {
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                // Other control characters (ASCII 0-31 except those already handled)
                c if c.is_control() => {
                    // Skip other control characters
                }
                _ => result.push(c),
            }
        } else {
            result.push(c);
        }
        prev_char = c;
    }

    result
}

/// Recursively search for a Recipe object in JSON-LD.
/// Handles @graph arrays and nested structures.
fn find_recipe_in_json(json: &serde_json::Value) -> Option<&serde_json::Value> {
    match json {
        serde_json::Value::Object(obj) => {
            // Check if this object is a Recipe
            if let Some(type_val) = obj.get("@type") {
                let is_recipe = match type_val {
                    serde_json::Value::String(s) => s == "Recipe",
                    serde_json::Value::Array(arr) => arr.iter().any(|v| v == "Recipe"),
                    _ => false,
                };
                if is_recipe {
                    return Some(json);
                }
            }

            // Check @graph for array of items
            if let Some(graph) = obj.get("@graph") {
                if let Some(recipe) = find_recipe_in_json(graph) {
                    return Some(recipe);
                }
            }

            // Recursively search other fields
            for (_, value) in obj {
                if let Some(recipe) = find_recipe_in_json(value) {
                    return Some(recipe);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let Some(recipe) = find_recipe_in_json(item) {
                    return Some(recipe);
                }
            }
        }
        _ => {}
    }
    None
}

/// Extract recipe data from a JSON-LD Recipe object.
fn extract_recipe_data(
    recipe: &serde_json::Value,
    source_url: &str,
) -> Result<RawRecipe, ExtractError> {
    let title = recipe
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ExtractError::MissingField("name".to_string()))?;
    let title = decode_html_entities(title);

    let description = recipe
        .get("description")
        .and_then(|v| v.as_str())
        .map(decode_html_entities);

    let ingredients = extract_ingredients(recipe)?;
    let instructions = extract_instructions(recipe)?;
    let image_urls = extract_image_urls(recipe);
    let source_name = extract_source_name(source_url);

    let servings = recipe.get("recipeYield").and_then(|v| match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(arr) => {
            arr.first().and_then(|v| v.as_str()).map(|s| s.to_string())
        }
        _ => None,
    });

    Ok(RawRecipe {
        title,
        description,
        ingredients: decode_html_entities(&ingredients),
        instructions: decode_html_entities(&instructions),
        image_urls,
        source_url: Some(source_url.to_string()),
        source_name,
        servings,
        prep_time: None,
        cook_time: None,
        total_time: None,
        rating: None,
        difficulty: None,
        nutritional_info: None,
        notes: None,
        categories: None,
        footnotes: None,
    })
}

/// Split a single potentially concatenated ingredient string into individual ingredients.
///
/// Some websites (notably Serious Eats) produce JSON-LD where multiple ingredients
/// are concatenated into a single array element with no separator. This detects
/// two boundary patterns:
///
/// 1. `)digit` — close parenthesis directly followed by a digit (always splits)
/// 2. `word(3+ letters)→digit` — ASCII word of 3+ letters ending directly before a digit,
///    excluding 'x'/'X' before digit (avoids "4x175g"), validated by checking that the
///    text from the digit onward starts a new ingredient quantity.
fn split_concatenated_ingredient(s: &str) -> Vec<String> {
    let s = s.trim();
    if s.is_empty() {
        return Vec::new();
    }

    // Collect (byte_offset, char) pairs so we can look back at previous characters
    // while tracking byte positions for safe slicing via str::get().
    let indexed: Vec<(usize, u8)> = s.bytes().enumerate().collect();

    let mut split_positions: Vec<usize> = Vec::new();

    for idx in 1..indexed.len() {
        let (byte_pos, curr) = indexed[idx];
        if !curr.is_ascii_digit() {
            continue;
        }
        let (_, prev) = indexed[idx - 1];

        // Pattern 1: ) followed by digit — split if the digit starts a new quantity
        if prev == b')' {
            if let Some(rest) = s.get(byte_pos..) {
                if INGREDIENT_START_AFTER_SPLIT_RE.is_match(rest) {
                    split_positions.push(byte_pos);
                }
            }
            continue;
        }

        // Pattern 2: letter (not x/X) followed by digit, with 3+ letter word
        if prev.is_ascii_alphabetic()
            && prev != b'x'
            && prev != b'X'
            && idx >= 3
            && indexed[idx - 2].1.is_ascii_alphabetic()
            && indexed[idx - 3].1.is_ascii_alphabetic()
        {
            if let Some(rest) = s.get(byte_pos..) {
                if INGREDIENT_START_AFTER_SPLIT_RE.is_match(rest) {
                    split_positions.push(byte_pos);
                }
            }
        }
    }

    if split_positions.is_empty() {
        return vec![s.to_string()];
    }

    let mut parts = Vec::new();
    let mut start = 0;
    for &pos in &split_positions {
        if let Some(part) = s.get(start..pos) {
            let part = part.trim();
            if !part.is_empty() {
                parts.push(part.to_string());
            }
        }
        start = pos;
    }
    if let Some(part) = s.get(start..) {
        let part = part.trim();
        if !part.is_empty() {
            parts.push(part.to_string());
        }
    }

    parts
}

/// Apply concatenation splitting to each ingredient and flatten.
fn split_and_dedup_ingredients(ingredients: Vec<String>) -> Vec<String> {
    ingredients
        .into_iter()
        .flat_map(|s| split_concatenated_ingredient(&s))
        .collect()
}

/// Extract ingredients as a newline-separated blob.
fn extract_ingredients(recipe: &serde_json::Value) -> Result<String, ExtractError> {
    let ingredients_raw = recipe
        .get("recipeIngredient")
        .ok_or_else(|| ExtractError::MissingField("recipeIngredient".to_string()))?;

    let ingredients_array = ingredients_raw
        .as_array()
        .ok_or_else(|| ExtractError::InvalidJson("recipeIngredient is not an array".to_string()))?;

    let ingredients: Vec<String> = ingredients_array
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| decode_html_entities(s.trim()))
        .filter(|s| !s.is_empty())
        .collect();

    if ingredients.is_empty() {
        return Err(ExtractError::MissingField(
            "recipeIngredient (empty)".to_string(),
        ));
    }

    // Split concatenated ingredients and deduplicate
    let ingredients = split_and_dedup_ingredients(ingredients);

    Ok(ingredients.join("\n"))
}

/// Extract instructions from recipeInstructions field.
/// Handles both string and array formats.
fn extract_instructions(recipe: &serde_json::Value) -> Result<String, ExtractError> {
    let instructions_raw = recipe
        .get("recipeInstructions")
        .ok_or_else(|| ExtractError::MissingField("recipeInstructions".to_string()))?;

    match instructions_raw {
        serde_json::Value::String(s) => Ok(s.trim().to_string()),
        serde_json::Value::Array(arr) => {
            let steps: Vec<String> = arr
                .iter()
                .filter_map(|item| {
                    // Handle HowToStep objects
                    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                        return Some(text.trim().to_string());
                    }
                    // Handle plain strings
                    if let Some(s) = item.as_str() {
                        return Some(s.trim().to_string());
                    }
                    // Handle HowToSection with itemListElement
                    if let Some(items) = item.get("itemListElement").and_then(|v| v.as_array()) {
                        let section_steps: Vec<String> = items
                            .iter()
                            .filter_map(|step| step.get("text").and_then(|v| v.as_str()))
                            .map(|s| s.trim().to_string())
                            .collect();
                        if !section_steps.is_empty() {
                            return Some(section_steps.join("\n"));
                        }
                    }
                    None
                })
                .collect();

            if steps.is_empty() {
                return Err(ExtractError::MissingField(
                    "recipeInstructions (empty)".to_string(),
                ));
            }

            Ok(steps.join("\n\n"))
        }
        _ => Err(ExtractError::InvalidJson(
            "recipeInstructions is not a string or array".to_string(),
        )),
    }
}

/// Extract image URLs from the recipe.
fn extract_image_urls(recipe: &serde_json::Value) -> Vec<String> {
    let mut urls = Vec::new();

    if let Some(image) = recipe.get("image") {
        match image {
            serde_json::Value::String(s) => {
                urls.push(s.clone());
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    if let Some(s) = item.as_str() {
                        urls.push(s.to_string());
                    } else if let Some(obj) = item.as_object() {
                        if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
                            urls.push(url.to_string());
                        }
                    }
                }
            }
            serde_json::Value::Object(obj) => {
                if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
                    urls.push(url.to_string());
                }
            }
            _ => {}
        }
    }

    urls
}

/// Extract a friendly source name from a URL.
fn extract_source_name(url: &str) -> Option<String> {
    url::Url::parse(url).ok().and_then(|parsed| {
        parsed.host_str().map(|host| {
            // Remove www. prefix
            let name = host.strip_prefix("www.").unwrap_or(host);
            // Capitalize first letter
            let mut chars = name.chars();
            match chars.next() {
                None => name.to_string(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
    })
}

/// Extract recipe from schema.org microdata markup.
/// This is a fallback for sites that don't use JSON-LD but have microdata attributes.
fn extract_recipe_from_microdata(
    document: &Html,
    source_url: &str,
) -> Result<RawRecipe, ExtractError> {
    // Find the Recipe container element
    // Try both http and https schema.org URLs
    let recipe_selector = Selector::parse(
        r#"[itemtype="http://schema.org/Recipe"], [itemtype="https://schema.org/Recipe"]"#,
    )
    .expect("Invalid selector");

    let recipe_element = document
        .select(&recipe_selector)
        .next()
        .ok_or(ExtractError::NoRecipe)?;

    // Extract title from itemprop="name"
    let title = extract_microdata_text(&recipe_element, "name")
        .ok_or_else(|| ExtractError::MissingField("name".to_string()))?;
    let title = decode_html_entities(&title);

    // Extract description (optional)
    let description =
        extract_microdata_text(&recipe_element, "description").map(|s| decode_html_entities(&s));

    // Extract ingredients
    let ingredient_selector =
        Selector::parse(r#"[itemprop="recipeIngredient"], [itemprop="ingredients"]"#)
            .expect("Invalid selector");
    let ingredients: Vec<String> = recipe_element
        .select(&ingredient_selector)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let ingredients = split_and_dedup_ingredients(ingredients);

    if ingredients.is_empty() {
        return Err(ExtractError::MissingField(
            "recipeIngredient (empty)".to_string(),
        ));
    }

    // Extract instructions
    let instructions = extract_microdata_instructions(&recipe_element)?;

    // Extract image URLs
    let mut image_urls = extract_microdata_images(&recipe_element);
    // Fallback to og:image if no images found in microdata
    if image_urls.is_empty() {
        if let Some(og_image) = extract_og_image(document) {
            image_urls.push(og_image);
        }
    }

    let source_name = extract_source_name(source_url);
    let servings = extract_microdata_text(&recipe_element, "recipeYield");

    let ingredients_str = decode_html_entities(&ingredients.join("\n"));
    let footnotes = if ingredients_str.contains('*') {
        extract_footnotes_from_document(document)
    } else {
        None
    };

    Ok(RawRecipe {
        title,
        description,
        ingredients: ingredients_str,
        instructions: decode_html_entities(&instructions),
        image_urls,
        source_url: Some(source_url.to_string()),
        source_name,
        servings,
        prep_time: None,
        cook_time: None,
        total_time: None,
        rating: None,
        difficulty: None,
        nutritional_info: None,
        notes: None,
        categories: None,
        footnotes,
    })
}

/// Extract text content from an element with the given itemprop.
fn extract_microdata_text(element: &scraper::ElementRef, prop: &str) -> Option<String> {
    let selector = Selector::parse(&format!(r#"[itemprop="{}"]"#, prop)).ok()?;
    element.select(&selector).next().map(|el| {
        // Check for content attribute first (common for meta tags)
        if let Some(content) = el.value().attr("content") {
            content.trim().to_string()
        } else {
            el.text().collect::<String>().trim().to_string()
        }
    })
}

/// Extract instructions from microdata.
fn extract_microdata_instructions(
    recipe_element: &scraper::ElementRef,
) -> Result<String, ExtractError> {
    // Try to find instruction elements using schema.org microdata
    let step_selector = Selector::parse(
        r#"[itemprop="recipeInstructions"], [itemprop="instructions"], [itemtype*="HowToStep"]"#,
    )
    .expect("Invalid selector");

    let steps: Vec<String> = recipe_element
        .select(&step_selector)
        .map(|el| {
            // Check for text property inside HowToStep
            let text_selector = Selector::parse(r#"[itemprop="text"]"#).ok();
            if let Some(selector) = text_selector {
                if let Some(text_el) = el.select(&selector).next() {
                    return text_el.text().collect::<String>().trim().to_string();
                }
            }
            el.text().collect::<String>().trim().to_string()
        })
        .filter(|s| !s.is_empty())
        .collect();

    if !steps.is_empty() {
        return Ok(steps.join("\n\n"));
    }

    // Fallback: Try h-recipe microformat class (used by Jetpack and others)
    // Look for elements with class containing "instructions" or "directions"
    let class_selector = Selector::parse(
        r#".e-instructions, .instructions, .recipe-instructions, .jetpack-recipe-directions, .recipe-directions"#,
    )
    .expect("Invalid selector");

    let instructions: Vec<String> = recipe_element
        .select(&class_selector)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if !instructions.is_empty() {
        return Ok(instructions.join("\n\n"));
    }

    Err(ExtractError::MissingField(
        "recipeInstructions (empty)".to_string(),
    ))
}

/// Extract image URLs from microdata.
fn extract_microdata_images(recipe_element: &scraper::ElementRef) -> Vec<String> {
    let image_selector = Selector::parse(r#"[itemprop="image"]"#).expect("Invalid selector");

    recipe_element
        .select(&image_selector)
        .filter_map(|el| {
            // Check src attribute for img tags
            if let Some(src) = el.value().attr("src") {
                return Some(src.to_string());
            }
            // Check href attribute for link tags
            if let Some(href) = el.value().attr("href") {
                return Some(href.to_string());
            }
            // Check content attribute for meta tags
            if let Some(content) = el.value().attr("content") {
                return Some(content.to_string());
            }
            None
        })
        .collect()
}

/// Extract image URL from og:image meta tag.
/// This is a fallback for sites that don't include image data in their recipe structured data
/// (e.g., smittenkitchen.com uses Jetpack recipes which omit itemprop="image").
fn extract_og_image(document: &Html) -> Option<String> {
    let selector = Selector::parse(r#"meta[property="og:image"]"#).ok()?;
    document
        .select(&selector)
        .next()?
        .value()
        .attr("content")
        .map(|s| s.to_string())
}

/// Regex to match footnote lines starting with asterisks inside `<li>` or `<p>` tags.
/// Captures: (1) the asterisk marker, (2) the footnote text (may contain inline HTML like <em>).
static FOOTNOTE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<(?:li|p)[^>]*>\s*(\*{1,3})\s*(.{10,500}?)</(?:li|p)>")
        .expect("Invalid footnote regex")
});

/// Phrases that indicate a false-positive footnote (nutritional disclaimers, affiliate notices).
const FOOTNOTE_FALSE_POSITIVE_PREFIXES: &[&str] = &[
    "percent daily value",
    "daily values",
    "this post may contain affiliate",
    "this post contains affiliate",
    "nutrient information is not available",
];

/// Extract footnotes from HTML recipe notes sections.
///
/// Searches for `<li>` and `<p>` elements starting with `*`, `**`, or `***` inside
/// known recipe card note containers (WPRM, Tasty Recipes, etc.).
/// Returns a list of (marker, text) pairs, or None if no footnotes found.
pub fn extract_footnotes_from_html(html: &str) -> Option<Vec<(String, String)>> {
    // Scope the search to the portion of HTML starting from the first recipe notes
    // container. This avoids matching starred <li>/<p> elements in unrelated parts
    // of the page (sidebars, comments) while handling nested <div> elements that
    // break regex-based container content extraction.
    let search_html = find_notes_section_start(html)?;

    let candidates = FOOTNOTE_REGEX.captures_iter(search_html).map(|cap| {
        let marker = cap.get(1).unwrap().as_str().to_string();
        // Strip inline HTML tags (e.g., <em>, <strong>, <a>) from footnote text
        let raw_text = cap.get(2).unwrap().as_str().trim();
        let text = HTML_TAG_REGEX.replace_all(raw_text, "").trim().to_string();
        (marker, decode_html_entities(&text))
    });

    collect_footnotes(candidates)
}

/// Extract footnotes from a pre-parsed HTML document.
/// Uses CSS selectors instead of regex to avoid re-parsing the DOM.
fn extract_footnotes_from_document(document: &Html) -> Option<Vec<(String, String)>> {
    let notes_selector = Selector::parse(
        ".wprm-recipe-notes-container li, .wprm-recipe-notes-container p, .wprm-recipe-notes li, .wprm-recipe-notes p, .tasty-recipes-notes li, .tasty-recipes-notes p, .tasty-recipe-notes li, .tasty-recipe-notes p",
    ).ok()?;

    let candidates = document.select(&notes_selector).filter_map(|element| {
        let text: String = element.text().collect::<String>();
        let text = text.trim().to_string();

        let marker_len = text.chars().take_while(|&c| c == '*').count();
        if marker_len == 0 || marker_len > 3 {
            return None;
        }

        let marker = "*".repeat(marker_len);
        // Safe: '*' is ASCII so marker_len bytes == marker_len chars
        let footnote_text = text.get(marker_len..)?.trim();
        if footnote_text.len() < 10 {
            return None;
        }

        Some((marker, decode_html_entities(footnote_text)))
    });

    collect_footnotes(candidates)
}

/// Shared logic for deduplicating and filtering footnote candidates.
/// Takes an iterator of (marker, text) pairs and returns the first footnote
/// for each marker level, skipping false positives (nutritional disclaimers, etc.).
fn collect_footnotes(
    candidates: impl Iterator<Item = (String, String)>,
) -> Option<Vec<(String, String)>> {
    let mut footnotes: Vec<(String, String)> = Vec::new();
    let mut seen_markers = std::collections::HashSet::new();

    for (marker, text) in candidates {
        let lower = text.to_lowercase();
        if FOOTNOTE_FALSE_POSITIVE_PREFIXES
            .iter()
            .any(|prefix| lower.starts_with(prefix))
        {
            continue;
        }

        // Only keep the first footnote for each marker level
        if !seen_markers.contains(&marker) {
            seen_markers.insert(marker.clone());
            footnotes.push((marker, text));
        }
    }

    if footnotes.is_empty() {
        None
    } else {
        Some(footnotes)
    }
}

/// Regex to find a notes container class in an actual HTML class attribute,
/// ignoring matches inside `<style>` or `<script>` blocks.
static NOTES_CONTAINER_ATTR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)class\s*=\s*["'][^"']*(wprm-recipe-notes|tasty-recipes?-notes)[^"']*["']"#)
        .expect("Invalid notes container attr regex")
});

/// Find the start of the recipe notes section in HTML.
/// Returns a slice from the first notes container class attribute onwards,
/// scoping footnote search to avoid matching class names in `<style>`/`<script>` blocks.
fn find_notes_section_start(html: &str) -> Option<&str> {
    NOTES_CONTAINER_ATTR_REGEX
        .find(html)
        .and_then(|m| html.get(m.start()..))
}

/// Regex to strip HTML tags for extracting text from raw HTML fragments.
static HTML_TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<[^>]+>").expect("Invalid HTML tag regex"));

/// Regex to split HTML on paragraph boundaries for unstructured blog extraction.
static P_TAG_SPLIT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)</?p[^>]*>").expect("Invalid p-tag split regex"));

/// Regex to match `<br>` tags in various forms.
static BR_TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").expect("Invalid br regex"));

/// Regex to extract bold/strong text (recipe title signal).
static BOLD_TEXT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<(?:b|strong)>([^<]+)</(?:b|strong)>").expect("Invalid bold text regex")
});

/// Regex to extract underlined text (section header signal).
static UNDERLINE_TEXT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<u>([^<]+)</u>").expect("Invalid underline text regex"));

/// Regex to detect "One year ago:" / "Previously" / "Two years ago:" link sections.
static LOOKBACK_LINK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(\s*<b>)?\s*(one|two|three|four|five|six|seven|eight|nine|ten|\d+)\s+years?\s+ago\s*:|(^|\s)previously\b")
        .expect("Invalid lookback link regex")
});

/// Extract a recipe from an unstructured blog post.
///
/// Handles older WordPress posts that write recipes in plain HTML without any
/// recipe plugin or structured data. The defining signal is `<br>`-delimited
/// ingredient lists in `<p>` blocks: this is how bloggers commonly format
/// ingredient lists in the post editor.
///
/// Pattern:
/// 1. `<p><b>Recipe Title</b>...` (bold text introduces the recipe)
/// 2. `<p>ingredient 1<br>ingredient 2<br>ingredient 3</p>` (ingredients)
/// 3. `<p>Prose instruction paragraph.</p>` (instructions, no `<br>` chains)
fn extract_recipe_from_unstructured_blog(html: &str, source_url: &str) -> Option<RawRecipe> {
    // Limit search to before comments section to avoid picking up user comments.
    // These markers are ASCII so the byte position is always a valid char boundary.
    let comments_pos = html
        .find("<div id=\"comments\"")
        .or_else(|| html.find("<section id=\"comments\""))
        .or_else(|| html.find("<ol class=\"commentlist\""))
        .or_else(|| html.find("<div class=\"comments-area\""));
    let search_html = match comments_pos {
        Some(pos) => html.get(..pos).unwrap_or(html),
        None => html,
    };

    // Split on <p> tags to get paragraph chunks
    let chunks: Vec<&str> = P_TAG_SPLIT_REGEX.split(search_html).collect();

    // Find paragraph chunks that look like ingredient lists:
    // they contain 2+ <br> tags and their lines look like ingredients (short, with quantities)
    let ingredient_chunk_indices: Vec<usize> = chunks
        .iter()
        .enumerate()
        .filter(|(_, chunk)| {
            let trimmed = chunk.trim();
            !trimmed.is_empty()
                && BR_TAG_REGEX.find_iter(trimmed).count() >= 2
                && looks_like_ingredient_list(trimmed)
        })
        .map(|(i, _)| i)
        .collect();

    if ingredient_chunk_indices.is_empty() {
        return None;
    }

    let first_ingredient_idx = ingredient_chunk_indices[0];
    let last_ingredient_idx = *ingredient_chunk_indices.last().unwrap();

    // Extract ingredients from all identified ingredient chunks
    let mut ingredient_lines: Vec<String> = Vec::new();
    for &idx in &ingredient_chunk_indices {
        let chunk = chunks[idx];
        extract_ingredient_lines_from_chunk(chunk, &mut ingredient_lines);
    }

    if ingredient_lines.is_empty() {
        return None;
    }

    // Find recipe title: look backwards from the first ingredient chunk
    // for the nearest chunk containing a <b> or <strong> tag
    let mut title: Option<String> = None;
    for i in (0..first_ingredient_idx).rev() {
        let chunk = chunks[i].trim();
        if chunk.is_empty() {
            continue;
        }
        if let Some(cap) = BOLD_TEXT_REGEX.captures(chunk) {
            let bold_text = cap.get(1).unwrap().as_str().trim();
            // Skip "One year ago:" and similar navigational bold text
            if !LOOKBACK_LINK_REGEX.is_match(chunk) && !bold_text.is_empty() {
                title = Some(decode_html_entities(bold_text));
                break;
            }
        }
    }

    // Fall back to page title if no bold title found near ingredients
    if title.is_none() {
        let document = Html::parse_document(html);
        title = extract_title_from_html(&document);
    }

    let title = title?;

    // Extract instructions: prose paragraphs after the last ingredient chunk
    // that don't contain <br> chains (i.e., they're not ingredient lists)
    let mut instruction_paragraphs: Vec<String> = Vec::new();
    for chunk in chunks.iter().skip(last_ingredient_idx + 1) {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }

        // Stop at sharing/social buttons
        if chunk.contains("sharedaddy") || chunk.contains("sd-sharing") {
            break;
        }

        // Skip chunks that are just images or links with no text content
        let text = HTML_TAG_REGEX.replace_all(chunk, "");
        let text = text.trim();
        if text.is_empty() {
            continue;
        }

        // Skip "One year ago:" / "Previously" link sections
        if LOOKBACK_LINK_REGEX.is_match(chunk) {
            continue;
        }

        // Skip attribution lines at the start (before any real instructions)
        if instruction_paragraphs.is_empty() {
            let lower = text.to_lowercase();
            if lower.starts_with("adapted from")
                || lower.starts_with("from ")
                || lower.starts_with("recipe from")
                || lower.starts_with("source:")
            {
                continue;
            }
        }

        let decoded = decode_html_entities(text);
        if !decoded.is_empty() {
            instruction_paragraphs.push(decoded);
        }
    }

    if instruction_paragraphs.is_empty() {
        return None;
    }

    // Extract servings from chunks near the title (between title and first ingredient)
    let mut servings: Option<String> = None;
    for i in (0..first_ingredient_idx).rev() {
        let chunk = chunks[i].trim();
        if chunk.is_empty() {
            continue;
        }
        let text = HTML_TAG_REGEX.replace_all(chunk, "");
        let text = text.trim().to_lowercase();
        if text.starts_with("makes ") || text.starts_with("serves ") || text.starts_with("yield") {
            servings = Some(decode_html_entities(text.trim()));
            break;
        }
        // Only look back a couple chunks from ingredients
        if first_ingredient_idx - i > 3 {
            break;
        }
    }

    let image_urls = extract_og_image_fast(html).into_iter().collect();
    let source_name = extract_source_name(source_url);

    let ingredients_str = ingredient_lines.join("\n");
    let footnotes = if ingredients_str.contains('*') {
        extract_footnotes_from_html(html)
    } else {
        None
    };

    Some(RawRecipe {
        title,
        description: None,
        ingredients: ingredients_str,
        instructions: instruction_paragraphs.join("\n\n"),
        image_urls,
        source_url: Some(source_url.to_string()),
        source_name,
        servings,
        prep_time: None,
        cook_time: None,
        total_time: None,
        rating: None,
        difficulty: None,
        nutritional_info: None,
        notes: None,
        categories: None,
        footnotes,
    })
}

/// Regex to detect ingredient-like quantity patterns at the start of a line.
static INGREDIENT_QUANTITY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(\d|½|⅓|¼|⅔|¾|⅛|a\s+(pinch|few|handful|dash|splash)|juice\s+of|zest\s+of|pinch\s+of|dash\s+of|kosher\s+salt|salt[,\s]|ground\s|fresh\s|sea\s+salt)")
        .expect("Invalid ingredient quantity regex")
});

/// Check whether an HTML chunk looks like an ingredient list.
/// Ingredient paragraphs have multiple short lines (split by `<br>`) where
/// at least some lines contain quantity-like patterns (digits, fractions)
/// and lines are generally short (not prose paragraphs).
fn looks_like_ingredient_list(chunk: &str) -> bool {
    let lines: Vec<&str> = BR_TAG_REGEX.split(chunk).collect();
    if lines.len() < 2 {
        return false;
    }

    let mut quantity_lines = 0;
    let mut total_text_lines = 0;
    let mut long_lines = 0;

    for line in &lines {
        let text = HTML_TAG_REGEX.replace_all(line, "");
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        total_text_lines += 1;

        if text.len() > 200 {
            long_lines += 1;
        }

        // A line looks like an ingredient if it matches quantity patterns
        if text.len() < 300 && INGREDIENT_QUANTITY_REGEX.is_match(text) {
            quantity_lines += 1;
        }
    }

    // Reject chunks where most lines are very long (likely prose, not ingredients)
    if total_text_lines > 0 && (long_lines * 100 / total_text_lines) > 50 {
        return false;
    }

    // At least 2 text lines and at least 40% look like quantities
    total_text_lines >= 2 && quantity_lines > 0 && (quantity_lines * 100 / total_text_lines) >= 40
}

/// Extract individual ingredient lines from a `<br>`-delimited HTML chunk.
/// Preserves `<u>` section headers as their own lines.
fn extract_ingredient_lines_from_chunk(chunk: &str, lines: &mut Vec<String>) {
    for part in BR_TAG_REGEX.split(chunk) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        // Check for section header (<u> tag)
        if let Some(cap) = UNDERLINE_TEXT_REGEX.captures(part) {
            let header = cap.get(1).unwrap().as_str().trim();
            if !header.is_empty() {
                // Colon-terminate so the ingredient parser's section-header
                // detector picks it up (parallel to WPRM/Jetpack group headers).
                let decoded = decode_html_entities(header);
                if decoded.ends_with(':') {
                    lines.push(decoded);
                } else {
                    lines.push(format!("{}:", decoded));
                }
            }
            // There might be ingredient text after the </u> on the same line
            let after_u = UNDERLINE_TEXT_REGEX.replace(part, "");
            let after_text = HTML_TAG_REGEX.replace_all(&after_u, "");
            let after_text = after_text.trim();
            if !after_text.is_empty() {
                lines.push(decode_html_entities(after_text));
            }
        } else {
            let text = HTML_TAG_REGEX.replace_all(part, "");
            let text = text.trim();
            if !text.is_empty() {
                lines.push(decode_html_entities(text));
            }
        }
    }
}

/// Decode HTML entities using the html-escape crate.
/// Also handles double-encoded entities like "&amp;#8531;" by decoding twice.
fn decode_html_entities(text: &str) -> String {
    // First pass: decode entities (this handles &amp; -> & among others)
    let decoded = html_escape::decode_html_entities(text);
    // Second pass: decode again to handle double-encoded entities
    // e.g., "&amp;#8531;" -> "&#8531;" -> "⅓"
    let decoded = html_escape::decode_html_entities(&decoded);
    decoded.into_owned()
}

/// Extract a recipe by combining partial structured data with HTML class-based fallbacks.
/// This handles cases where JSON-LD or microdata has a Recipe object but with empty required
/// fields, while the actual content exists in HTML elements with common recipe plugin classes.
fn extract_recipe_with_html_fallback(
    html: &str,
    document: &Html,
    source_url: &str,
) -> Result<RawRecipe, ExtractError> {
    // Try to get partial data from JSON-LD
    let partial = extract_partial_from_jsonld(html);

    // Try to get partial data from microdata
    let micro_partial = extract_partial_from_microdata(document);

    // Merge: prefer JSON-LD fields, fall back to microdata
    let title = partial.title.or(micro_partial.title);
    let description = partial.description.or(micro_partial.description);
    let ingredients = partial.ingredients.or(micro_partial.ingredients);
    let instructions = partial.instructions.or(micro_partial.instructions);
    let image_urls = if partial.image_urls.is_empty() {
        micro_partial.image_urls
    } else {
        partial.image_urls
    };
    let servings = partial.servings.or(micro_partial.servings);

    // For any still-missing required fields, try HTML class-based fallbacks
    let title = title.or_else(|| extract_title_from_html(document));

    let ingredients = ingredients
        .or_else(|| extract_ingredients_from_html_classes(document))
        .or_else(|| extract_ingredients_from_itemprop_unscoped(document));

    let instructions = instructions
        .or_else(|| extract_instructions_from_html_classes(document))
        .or_else(|| extract_instructions_from_itemprop_unscoped(document))
        .or_else(|| extract_instructions_from_raw_html(html));

    // If we got all required fields from structured data / class-based fallbacks, use them
    if let (Some(title), Some(ingredients), Some(instructions)) =
        (title.clone(), ingredients.clone(), instructions.clone())
    {
        let mut image_urls = image_urls;
        if image_urls.is_empty() {
            if let Some(og_image) = extract_og_image(document) {
                image_urls.push(og_image);
            }
        }

        let source_name = extract_source_name(source_url);

        let footnotes = if ingredients.contains('*') {
            extract_footnotes_from_document(document)
        } else {
            None
        };

        return Ok(RawRecipe {
            title,
            description,
            ingredients,
            instructions,
            image_urls,
            source_url: Some(source_url.to_string()),
            source_name,
            servings,
            prep_time: None,
            cook_time: None,
            total_time: None,
            rating: None,
            difficulty: None,
            nutritional_info: None,
            notes: None,
            categories: None,
            footnotes,
        });
    }

    // Last resort: try unstructured blog recipe extraction (handles older WordPress
    // posts that write recipes in plain HTML without any recipe plugin)
    if let Some(recipe) = extract_recipe_from_unstructured_blog(html, source_url) {
        return Ok(recipe);
    }

    // Nothing worked — report what's missing
    if title.is_none() {
        return Err(ExtractError::MissingField("name".to_string()));
    }
    if ingredients.is_none() {
        return Err(ExtractError::MissingField(
            "recipeIngredient (empty)".to_string(),
        ));
    }
    Err(ExtractError::MissingField(
        "recipeInstructions (empty)".to_string(),
    ))
}

/// Partial recipe data extracted leniently (missing required fields are None, not errors).
struct PartialRecipe {
    title: Option<String>,
    description: Option<String>,
    ingredients: Option<String>,
    instructions: Option<String>,
    image_urls: Vec<String>,
    servings: Option<String>,
}

/// Extract whatever we can from JSON-LD without failing on missing required fields.
fn extract_partial_from_jsonld(html: &str) -> PartialRecipe {
    for cap in JSONLD_REGEX.captures_iter(html) {
        let json_text = match cap.get(1) {
            Some(m) => m.as_str(),
            None => continue,
        };

        let sanitized = sanitize_json(json_text);
        let json: serde_json::Value = match serde_json::from_str(&sanitized) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(recipe) = find_recipe_in_json(&json) {
            let title = recipe
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let description = recipe
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let ingredients = extract_ingredients(recipe).ok();
            let instructions = extract_instructions(recipe).ok();
            let image_urls = extract_image_urls(recipe);

            let servings = recipe.get("recipeYield").and_then(|v| match v {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Array(arr) => {
                    arr.first().and_then(|v| v.as_str()).map(|s| s.to_string())
                }
                _ => None,
            });

            return PartialRecipe {
                title,
                description,
                ingredients,
                instructions,
                image_urls,
                servings,
            };
        }
    }

    PartialRecipe {
        title: None,
        description: None,
        ingredients: None,
        instructions: None,
        image_urls: Vec::new(),
        servings: None,
    }
}

/// Extract whatever we can from microdata without failing on missing required fields.
fn extract_partial_from_microdata(document: &Html) -> PartialRecipe {
    let recipe_selector = Selector::parse(
        r#"[itemtype="http://schema.org/Recipe"], [itemtype="https://schema.org/Recipe"]"#,
    )
    .expect("Invalid selector");

    let recipe_element = match document.select(&recipe_selector).next() {
        Some(el) => el,
        None => {
            return PartialRecipe {
                title: None,
                description: None,
                ingredients: None,
                instructions: None,
                image_urls: Vec::new(),
                servings: None,
            }
        }
    };

    let title = extract_microdata_text(&recipe_element, "name");
    let description = extract_microdata_text(&recipe_element, "description");

    let ingredient_selector =
        Selector::parse(r#"[itemprop="recipeIngredient"], [itemprop="ingredients"]"#)
            .expect("Invalid selector");
    let ingredients_vec: Vec<String> = recipe_element
        .select(&ingredient_selector)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let ingredients_vec = split_and_dedup_ingredients(ingredients_vec);
    let ingredients = if ingredients_vec.is_empty() {
        None
    } else {
        Some(ingredients_vec.join("\n"))
    };

    let instructions = extract_microdata_instructions(&recipe_element).ok();

    let image_urls = extract_microdata_images(&recipe_element);
    let servings = extract_microdata_text(&recipe_element, "recipeYield");

    PartialRecipe {
        title,
        description,
        ingredients,
        instructions,
        image_urls,
        servings,
    }
}

/// Search the entire document for itemprop="recipeIngredient" elements,
/// regardless of whether they're inside an itemscope container.
/// Handles malformed HTML where ingredients fall outside the Recipe scope.
fn extract_ingredients_from_itemprop_unscoped(document: &Html) -> Option<String> {
    let selector = Selector::parse(r#"[itemprop="recipeIngredient"], [itemprop="ingredients"]"#)
        .expect("Invalid selector");
    let ingredients: Vec<String> = document
        .select(&selector)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let ingredients = split_and_dedup_ingredients(ingredients);

    if ingredients.is_empty() {
        None
    } else {
        Some(ingredients.join("\n"))
    }
}

/// Search the entire document for instruction elements via itemprop,
/// regardless of whether they're inside an itemscope container.
fn extract_instructions_from_itemprop_unscoped(document: &Html) -> Option<String> {
    let selector = Selector::parse(r#"[itemprop="recipeInstructions"], [itemprop="instructions"]"#)
        .expect("Invalid selector");

    let steps: Vec<String> = document
        .select(&selector)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if steps.is_empty() {
        None
    } else {
        Some(steps.join("\n\n"))
    }
}

/// Extract ingredients from common recipe plugin HTML classes.
/// Searches the entire document (not scoped to a microdata container).
fn extract_ingredients_from_html_classes(document: &Html) -> Option<String> {
    // Try Jetpack recipe ingredient list items (with group support)
    if let Some(result) = extract_jetpack_ingredients_with_groups(document) {
        return Some(result);
    }
    // Fallback: Jetpack without groups
    if let Some(result) =
        extract_ingredient_items_from_selector(document, ".jetpack-recipe-ingredient")
    {
        return Some(result);
    }

    // Try div.ingredients with <br>-separated content (mybakingaddiction old format)
    if let Some(result) = extract_ingredients_from_div(document) {
        return Some(result);
    }

    // Try WP Recipe Maker (with group support)
    if let Some(result) = extract_wprm_ingredients_with_groups(document) {
        return Some(result);
    }
    // Fallback: WPRM without groups
    if let Some(result) =
        extract_ingredient_items_from_selector(document, ".wprm-recipe-ingredient")
    {
        return Some(result);
    }

    // Try Tasty Recipes
    if let Some(result) =
        extract_ingredient_items_from_selector(document, ".tasty-recipe-ingredients li")
    {
        return Some(result);
    }

    None
}

/// Extract WPRM ingredients with group headers (e.g. "Meatballs:", "Broth:").
///
/// WPRM structures ingredients as `.wprm-recipe-ingredient-group` containers,
/// each with an optional `.wprm-recipe-group-name` header and a list of
/// `.wprm-recipe-ingredient` items. JSON-LD flattens these into a single array,
/// losing the group structure. This function recovers it from the HTML.
fn extract_wprm_ingredients_with_groups(document: &Html) -> Option<String> {
    let group_selector = Selector::parse(".wprm-recipe-ingredient-group").ok()?;
    let name_selector = Selector::parse(".wprm-recipe-group-name").ok()?;
    let item_selector = Selector::parse(".wprm-recipe-ingredient").ok()?;

    let groups: Vec<_> = document.select(&group_selector).collect();
    if groups.is_empty() {
        return None;
    }

    // Only use this path when at least one group actually has a name
    let has_any_group_name = groups.iter().any(|g| {
        g.select(&name_selector)
            .next()
            .map(|el| !el.text().collect::<String>().trim().is_empty())
            .unwrap_or(false)
    });
    if !has_any_group_name {
        return None;
    }

    let mut lines: Vec<String> = Vec::new();
    for group in &groups {
        // Extract group name if present
        if let Some(name_el) = group.select(&name_selector).next() {
            let name = name_el.text().collect::<String>().trim().to_string();
            if !name.is_empty() {
                // Add as section header (colon-terminated so the parser detects it)
                if name.ends_with(':') {
                    lines.push(name);
                } else {
                    lines.push(format!("{}:", name));
                }
            }
        }

        // Extract ingredients in this group
        for item in group.select(&item_selector) {
            let text = item.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                lines.push(text);
            }
        }
    }

    let lines = split_and_dedup_ingredients(lines);
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

/// Extract Jetpack recipe ingredients with group headers.
///
/// Jetpack structures ingredients inside a `.jetpack-recipe-ingredients` container
/// with `<h5>` (or other heading) elements as group headers interleaved with
/// `.jetpack-recipe-ingredient` list items. Microdata extraction only picks up
/// the `[itemprop]` items, losing the headings. This function walks the container's
/// children to recover the group structure.
fn extract_jetpack_ingredients_with_groups(document: &Html) -> Option<String> {
    let container_selector = Selector::parse(".jetpack-recipe-ingredients").ok()?;
    let container = document.select(&container_selector).next()?;

    let heading_selector = Selector::parse("h1, h2, h3, h4, h5, h6").ok()?;

    // Check if there are any headings inside the container
    let has_headings = container.select(&heading_selector).next().is_some();
    if !has_headings {
        return None;
    }

    // Walk all descendant elements in document order, emitting headings and
    // ingredient items as we encounter them.
    let mut lines: Vec<String> = Vec::new();
    let all_selector =
        Selector::parse("h1, h2, h3, h4, h5, h6, .jetpack-recipe-ingredient").ok()?;
    for el in container.select(&all_selector) {
        let tag = el.value().name();
        let text = el.text().collect::<String>().trim().to_string();
        if text.is_empty() {
            continue;
        }
        if tag.starts_with('h') && tag.len() == 2 {
            // Heading → section header
            if text.ends_with(':') {
                lines.push(text);
            } else {
                lines.push(format!("{}:", text));
            }
        } else {
            lines.push(text);
        }
    }

    let lines = split_and_dedup_ingredients(lines);
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

/// Extract ingredient items from a CSS selector, splitting concatenated entries and deduplicating.
fn extract_ingredient_items_from_selector(document: &Html, selector_str: &str) -> Option<String> {
    let selector = Selector::parse(selector_str).ok()?;
    let items: Vec<String> = document
        .select(&selector)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let items = split_and_dedup_ingredients(items);

    if items.is_empty() {
        None
    } else {
        Some(items.join("\n"))
    }
}

/// Extract ingredients from a `<div class="ingredients">` container.
/// Handles the old WordPress recipe format where ingredients are in `<p>` tags
/// separated by `<br>` elements, with optional `<h4>` section headers.
fn extract_ingredients_from_div(document: &Html) -> Option<String> {
    let selector = Selector::parse("div.ingredients").ok()?;
    let div = document.select(&selector).next()?;

    // Get the inner HTML and split on <br> tags to get individual lines
    let inner_html = div.inner_html();
    let mut lines: Vec<String> = Vec::new();

    // Split on <br>, <br/>, <br />, </p><p>, </p>, <p>
    for chunk in Regex::new(r"(?i)<br\s*/?>|</p>\s*<p>|</?p>")
        .expect("Invalid regex")
        .split(&inner_html)
    {
        // Strip remaining HTML tags and decode entities
        let text = HTML_TAG_REGEX.replace_all(chunk, "");
        let text = text.trim();
        if !text.is_empty() {
            // Decode common HTML entities
            let text = text
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&#8217;", "\u{2019}")
                .replace("&deg;", "\u{00b0}")
                .replace("&reg;", "\u{00ae}")
                .replace("&#038;", "&");
            if !text.is_empty() {
                lines.push(text);
            }
        }
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

/// Extract instructions from common recipe plugin HTML classes.
/// Searches the entire document (not scoped to a microdata container).
fn extract_instructions_from_html_classes(document: &Html) -> Option<String> {
    let selectors = [
        ".jetpack-recipe-directions",
        "div.instructions",
        ".recipe-instructions",
        ".e-instructions",
        ".wprm-recipe-instruction",
        ".recipe-directions",
    ];

    for selector_str in selectors {
        let selector = match Selector::parse(selector_str) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let steps: Vec<String> = document
            .select(&selector)
            .map(|el| el.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if !steps.is_empty() {
            return Some(steps.join("\n\n"));
        }
    }

    None
}

/// Regex-based extraction of instructions from raw HTML.
/// Handles malformed HTML where DOM parsing collapses the instruction container
/// (e.g., smittenkitchen's Jetpack recipes where `<p><div class="...">` causes
/// the div to close immediately, leaving instruction text as siblings).
///
/// In the raw HTML, the pattern is:
///   `<div class="jetpack-recipe-directions e-instructions"></p>`
///   `<p></div><br />`
///   [ACTUAL INSTRUCTIONS IN `<p>` TAGS]
///   `</div></div>`
/// We capture between the broken div close and the recipe container close.
static JETPACK_DIRECTIONS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<div[^>]*class="[^"]*jetpack-recipe-directions[^"]*"[^>]*>.*?</div>\s*(?:<br\s*/?>)?\s*(.*?)</div>\s*</div>"#,
    )
    .expect("Invalid Jetpack directions regex")
});

fn extract_instructions_from_raw_html(html: &str) -> Option<String> {
    if let Some(cap) = JETPACK_DIRECTIONS_REGEX.captures(html) {
        if let Some(content) = cap.get(1) {
            let raw = content.as_str();
            let paragraphs = html_to_paragraphs(raw);

            if !paragraphs.is_empty() {
                return Some(paragraphs.join("\n\n"));
            }
        }
    }
    None
}

/// Convert an HTML fragment into a list of plain-text paragraphs.
/// Splits on `</p><p>` and `<br><br>` boundaries, strips tags, decodes entities.
fn html_to_paragraphs(html: &str) -> Vec<String> {
    Regex::new(r"(?i)</p>\s*<p[^>]*>|<br\s*/?\s*>\s*<br\s*/?\s*>")
        .expect("Invalid paragraph split regex")
        .split(html)
        .map(|chunk| {
            let text = HTML_TAG_REGEX.replace_all(chunk, "");
            let text = text
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&#8217;", "\u{2019}")
                .replace("&#8220;", "\u{201c}")
                .replace("&#8221;", "\u{201d}")
                .replace("&#8212;", "\u{2014}")
                .replace("&#038;", "&")
                .replace("&deg;", "\u{00b0}")
                .replace("&reg;", "\u{00ae}");
            text.trim().to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// Extract a recipe title from common HTML elements when structured data lacks a name.
fn extract_title_from_html(document: &Html) -> Option<String> {
    // Try Jetpack recipe title
    let selectors = [
        ".jetpack-recipe-title",
        ".wprm-recipe-name",
        "h1.entry-title",
        "h2.entry-title",
    ];

    for selector_str in selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(el) = document.select(&selector).next() {
                let text = el.text().collect::<String>();
                let text = text.trim();
                if !text.is_empty() {
                    return Some(text.to_string());
                }
            }
        }
    }

    // Last resort: <title> tag, stripped of site name suffix
    let title_selector = Selector::parse("title").ok()?;
    let title_el = document.select(&title_selector).next()?;
    let title_text = title_el.text().collect::<String>();
    let title_text = title_text.trim();
    if title_text.is_empty() {
        return None;
    }
    // Strip common " - Site Name" or " | Site Name" suffixes
    let title = title_text
        .split(" - ")
        .next()
        .or_else(|| title_text.split(" | ").next())
        .unwrap_or(title_text)
        .trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_og_image_fallback_for_microdata_without_image() {
        // HTML with microdata recipe but no itemprop="image", only og:image
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <meta property="og:image" content="https://example.com/recipe-photo.jpg">
            </head>
            <body>
                <div itemscope itemtype="https://schema.org/Recipe">
                    <h1 itemprop="name">Test Recipe</h1>
                    <p itemprop="description">A test description</p>
                    <ul>
                        <li itemprop="recipeIngredient">1 cup flour</li>
                        <li itemprop="recipeIngredient">2 eggs</li>
                    </ul>
                    <div itemprop="recipeInstructions">Mix and bake.</div>
                </div>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();

        assert_eq!(result.title, "Test Recipe");
        assert_eq!(result.image_urls.len(), 1);
        assert_eq!(result.image_urls[0], "https://example.com/recipe-photo.jpg");
    }

    #[test]
    fn test_og_image_not_used_when_microdata_has_image() {
        // HTML with microdata recipe that HAS itemprop="image"
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <meta property="og:image" content="https://example.com/og-photo.jpg">
            </head>
            <body>
                <div itemscope itemtype="https://schema.org/Recipe">
                    <h1 itemprop="name">Test Recipe</h1>
                    <img itemprop="image" src="https://example.com/microdata-photo.jpg">
                    <ul>
                        <li itemprop="recipeIngredient">1 cup flour</li>
                    </ul>
                    <div itemprop="recipeInstructions">Mix and bake.</div>
                </div>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();

        // Should use the microdata image, not the og:image
        assert_eq!(result.image_urls.len(), 1);
        assert_eq!(
            result.image_urls[0],
            "https://example.com/microdata-photo.jpg"
        );
    }

    #[test]
    fn test_extract_og_image() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <meta property="og:image" content="https://example.com/image.jpg">
            </head>
            <body></body>
            </html>
        "#;

        let document = Html::parse_document(html);
        let og_image = extract_og_image(&document);

        assert_eq!(og_image, Some("https://example.com/image.jpg".to_string()));
    }

    #[test]
    fn test_extract_servings_from_jsonld_string() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Test Recipe",
                    "recipeYield": "4 servings",
                    "recipeIngredient": ["1 cup flour"],
                    "recipeInstructions": "Mix and bake."
                }
                </script>
            </head>
            <body></body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        assert_eq!(result.servings, Some("4 servings".to_string()));
    }

    #[test]
    fn test_extract_servings_from_jsonld_array() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Test Recipe",
                    "recipeYield": ["8 slices", "4 servings"],
                    "recipeIngredient": ["1 cup flour"],
                    "recipeInstructions": "Mix and bake."
                }
                </script>
            </head>
            <body></body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        assert_eq!(result.servings, Some("8 slices".to_string()));
    }

    #[test]
    fn test_extract_servings_from_microdata() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <div itemscope itemtype="https://schema.org/Recipe">
                    <h1 itemprop="name">Test Recipe</h1>
                    <span itemprop="recipeYield">Serves 6</span>
                    <ul>
                        <li itemprop="recipeIngredient">1 cup flour</li>
                    </ul>
                    <div itemprop="recipeInstructions">Mix and bake.</div>
                </div>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        assert_eq!(result.servings, Some("Serves 6".to_string()));
    }

    #[test]
    fn test_jsonld_empty_ingredients_falls_back_to_html_div() {
        // JSON-LD has empty recipeIngredient, but HTML has div.ingredients (mybakingaddiction pattern)
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Apple Bars",
                    "recipeIngredient": [],
                    "recipeInstructions": [{"@type": "HowToStep", "text": "Preheat oven to 350."}]
                }
                </script>
            </head>
            <body>
                <div class="ingredients">
                    <h4>For the Bars</h4>
                    <p>1 cup flour<br>2 eggs<br>1/2 cup sugar</p>
                    <h4>For the Glaze</h4>
                    <p>1 cup powdered sugar<br>1 tablespoon milk</p>
                </div>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        assert_eq!(result.title, "Apple Bars");
        assert!(result.ingredients.contains("1 cup flour"));
        assert!(result.ingredients.contains("2 eggs"));
        assert!(result.ingredients.contains("1 tablespoon milk"));
        assert!(result.ingredients.contains("For the Bars"));
    }

    #[test]
    fn test_microdata_ingredients_outside_scope_falls_back() {
        // Simulates smittenkitchen's malformed HTML where itemprop elements
        // fall outside the itemscope container due to premature div closure
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <div itemscope itemtype="https://schema.org/Recipe">
                    <h3 itemprop="name">Chicken Wonton Soup</h3>
                    <div class="jetpack-recipe-content"></div>
                </div>
                <!-- Ingredients are outside the itemscope due to malformed HTML -->
                <ul>
                    <li itemprop="recipeIngredient">3/4 pound ground chicken</li>
                    <li itemprop="recipeIngredient">1 teaspoon soy sauce</li>
                    <li itemprop="recipeIngredient">50 wonton wrappers</li>
                </ul>
                <div itemprop="recipeInstructions">Combine chicken and soy sauce in a bowl.</div>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://smittenkitchen.com/recipe").unwrap();
        assert_eq!(result.title, "Chicken Wonton Soup");
        assert!(result.ingredients.contains("3/4 pound ground chicken"));
        assert!(result.ingredients.contains("50 wonton wrappers"));
        assert!(result.instructions.contains("Combine chicken"));
    }

    #[test]
    fn test_html_fallback_jetpack_ingredients() {
        // Jetpack recipe with ingredients in .jetpack-recipe-ingredient class
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <div itemscope itemtype="https://schema.org/Recipe">
                    <h3 itemprop="name">Test Recipe</h3>
                    <div class="jetpack-recipe-content"></div>
                </div>
                <ul>
                    <li class="jetpack-recipe-ingredient">1 cup flour</li>
                    <li class="jetpack-recipe-ingredient">2 eggs</li>
                </ul>
                <div class="jetpack-recipe-directions">Mix and bake at 350.</div>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        assert_eq!(result.title, "Test Recipe");
        assert!(result.ingredients.contains("1 cup flour"));
        assert!(result.ingredients.contains("2 eggs"));
        assert!(result.instructions.contains("Mix and bake"));
    }

    #[test]
    fn test_html_fallback_title_from_entry_title() {
        // JSON-LD with missing name, title available in h1.entry-title
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "recipeIngredient": ["1 cup flour"],
                    "recipeInstructions": "Mix and bake."
                }
                </script>
            </head>
            <body>
                <h1 class="entry-title">My Great Recipe</h1>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        assert_eq!(result.title, "My Great Recipe");
    }

    #[test]
    fn test_unstructured_blog_basic_recipe() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <meta property="og:image" content="https://example.com/recipe.jpg">
            </head>
            <body>
                <h1 class="entry-title">My Blog Post About Cake</h1>
                <div class="entry-content">
                    <p>I love making this cake. It reminds me of childhood.</p>
                    <p><strong>Simple Vanilla Cake</strong></p>
                    <p>2 cups flour<br />1 cup sugar<br />3 eggs<br />1 cup milk<br />1 teaspoon vanilla extract</p>
                    <p>Preheat oven to 350. Mix dry ingredients. Add wet ingredients. Pour into pan and bake for 30 minutes.</p>
                </div>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.com/blog/cake").unwrap();
        assert_eq!(result.title, "Simple Vanilla Cake");
        assert!(result.ingredients.contains("2 cups flour"));
        assert!(result.ingredients.contains("3 eggs"));
        assert!(result.ingredients.contains("1 teaspoon vanilla extract"));
        assert!(result.instructions.contains("Preheat oven to 350"));
    }

    #[test]
    fn test_unstructured_blog_with_section_headers() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <p><b>Apple Pie</b></p>
                <p><u>For the crust</u><br />2 cups flour<br />1 stick butter<br />1/4 cup ice water</p>
                <p><u>For the filling</u><br />6 apples<br />1 cup sugar<br />1 teaspoon cinnamon</p>
                <p>Make the crust by cutting butter into flour. Roll out and place in pie dish.</p>
                <p>Slice apples and toss with sugar and cinnamon. Fill the crust and bake at 375 for 45 minutes.</p>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.com/pie").unwrap();
        assert_eq!(result.title, "Apple Pie");
        assert!(result.ingredients.contains("For the crust:"));
        assert!(result.ingredients.contains("2 cups flour"));
        assert!(result.ingredients.contains("For the filling:"));
        assert!(result.ingredients.contains("6 apples"));
        assert!(result.instructions.contains("Make the crust"));
    }

    #[test]
    fn test_unstructured_blog_u_headers_are_colon_terminated() {
        // `<u>` section headers must be emitted colon-terminated so the
        // ingredient parser can detect them as section markers. Otherwise
        // plain headers like "For the chicken" or "To assemble" (no colon
        // in the source HTML) get treated as ingredients.
        let html = r#"
            <html><body>
                <p><b>Fajitas</b></p>
                <p><u>For the chicken</u><br />1 pound chicken<br />1 teaspoon salt</p>
                <p><u>To assemble</u><br />8 tortillas<br />Olive oil<br />2 bell peppers</p>
                <p>Cook everything together until done.</p>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/fajitas").unwrap();
        assert!(
            result.ingredients.contains("For the chicken:"),
            "expected colon-terminated 'For the chicken:' header, got:\n{}",
            result.ingredients
        );
        assert!(
            result.ingredients.contains("To assemble:"),
            "expected colon-terminated 'To assemble:' header, got:\n{}",
            result.ingredients
        );
    }

    #[test]
    fn test_unstructured_blog_no_ingredients_returns_none() {
        // A blog post without br-delimited ingredient lists should not extract
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <p><b>My Trip to Paris</b></p>
                <p>We visited the Eiffel Tower and ate at a lovely bistro.</p>
                <p>The food was amazing and the views were spectacular.</p>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.com/paris");
        assert!(result.is_err());
    }

    #[test]
    fn test_html_fallback_with_stats_reports_method() {
        // Verify that extract_recipe_with_stats reports HtmlFallback method
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Test Recipe",
                    "recipeIngredient": [],
                    "recipeInstructions": "Mix it."
                }
                </script>
            </head>
            <body>
                <div class="ingredients"><p>1 cup flour<br>2 eggs</p></div>
            </body>
            </html>
        "#;

        let result = extract_recipe_with_stats(html, "https://example.com/recipe").unwrap();
        assert_eq!(result.method_used, ExtractionMethod::HtmlFallback);
        assert_eq!(result.raw_recipe.title, "Test Recipe");
        assert!(result.raw_recipe.ingredients.contains("1 cup flour"));
    }

    #[test]
    fn test_jsonld_decodes_html_entities() {
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Nami&#39;s Rice Crackers",
                    "description": "A delicious &amp; crunchy snack",
                    "recipeIngredient": ["1 cup flour", "&#189; tsp salt"],
                    "recipeInstructions": "When it&#39;s hot, add oil."
                }
                </script>
            </head><body></body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        assert_eq!(result.title, "Nami's Rice Crackers");
        assert_eq!(
            result.description.as_deref(),
            Some("A delicious & crunchy snack")
        );
        assert!(
            result.ingredients.contains("\u{00bd} tsp salt"),
            "got: {}",
            result.ingredients
        );
        assert!(
            result.instructions.contains("it's hot"),
            "got: {}",
            result.instructions
        );
    }

    #[test]
    fn test_microdata_decodes_html_entities() {
        let html = r#"
            <!DOCTYPE html>
            <html><body>
                <div itemscope itemtype="https://schema.org/Recipe">
                    <h1 itemprop="name">Nami&#39;s Dango</h1>
                    <p itemprop="description">Sweet &amp; chewy</p>
                    <ul>
                        <li itemprop="recipeIngredient">1 cup flour</li>
                    </ul>
                    <div itemprop="recipeInstructions">When it&#39;s done, serve.</div>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        assert_eq!(result.title, "Nami's Dango");
        assert_eq!(result.description.as_deref(), Some("Sweet & chewy"));
        assert!(
            result.instructions.contains("it's done"),
            "got: {}",
            result.instructions
        );
    }

    // --- Concatenated ingredient splitting tests ---

    #[test]
    fn test_split_paren_digit() {
        let input =
            "4 large eggs (200g)300g granulated sugar (10 1/2 ounces; 1 1/2 cups)192g cake flour";
        let result = split_concatenated_ingredient(input);
        assert_eq!(
            result,
            vec![
                "4 large eggs (200g)",
                "300g granulated sugar (10 1/2 ounces; 1 1/2 cups)",
                "192g cake flour",
            ]
        );
    }

    #[test]
    fn test_split_word_digit_with_unit() {
        let input = "1/2 teaspoon baking soda2 teaspoons vanilla extract288g all-purpose flour (10 ounces; 2 1/4 cups)";
        let result = split_concatenated_ingredient(input);
        assert_eq!(
            result,
            vec![
                "1/2 teaspoon baking soda",
                "2 teaspoons vanilla extract",
                "288g all-purpose flour (10 ounces; 2 1/4 cups)",
            ]
        );
    }

    #[test]
    fn test_split_milk_digit() {
        let input = "45g (3 tablespoons) cold whole milk1 large egg yolk";
        let result = split_concatenated_ingredient(input);
        assert_eq!(
            result,
            vec!["45g (3 tablespoons) cold whole milk", "1 large egg yolk",]
        );
    }

    #[test]
    fn test_split_no_false_positive_a1() {
        // "A1" is only 1 letter before digit — should NOT split
        let input = "2 Tbsp A1 Steak Sauce";
        let result = split_concatenated_ingredient(input);
        assert_eq!(result, vec!["2 Tbsp A1 Steak Sauce"]);
    }

    #[test]
    fn test_split_no_false_positive_x_multiplier() {
        // "4x175g" has 'x' before digit — should NOT split
        let input = "4 4x175g/6oz firm skinless white fish fillets";
        let result = split_concatenated_ingredient(input);
        assert_eq!(
            result,
            vec!["4 4x175g/6oz firm skinless white fish fillets"]
        );
    }

    #[test]
    fn test_split_no_false_positive_fraction_entity() {
        // After HTML decoding, ½ is a Unicode character, not "frac12"
        let input = "1 \u{00bd} cups flour";
        let result = split_concatenated_ingredient(input);
        assert_eq!(result, vec!["1 \u{00bd} cups flour"]);
    }

    #[test]
    fn test_split_normal_ingredient_unchanged() {
        let input = "1 cup (240ml) all-purpose flour";
        let result = split_concatenated_ingredient(input);
        assert_eq!(result, vec!["1 cup (240ml) all-purpose flour"]);
    }

    #[test]
    fn test_split_empty_string() {
        let result = split_concatenated_ingredient("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_split_preserves_duplicate_ingredients() {
        // Intentional duplicates (same ingredient in different recipe sections) must survive
        let input = vec![
            "4 large eggs (200g)300g sugar".to_string(),
            "300g sugar".to_string(),
        ];
        let result = split_and_dedup_ingredients(input);
        assert_eq!(
            result,
            vec!["4 large eggs (200g)", "300g sugar", "300g sugar"]
        );
    }

    #[test]
    fn test_split_paren_digit_requires_quantity() {
        // ")2" where the digit doesn't start a quantity should NOT split
        let input = "1 cup (240ml)2% milk";
        let result = split_concatenated_ingredient(input);
        assert_eq!(result, vec!["1 cup (240ml)2% milk"]);
    }

    #[test]
    fn test_split_eton_mess_severe_concatenation() {
        let input = "1 pound (454g) strawberries, washed, hulled, and quartered8 ounces (227g) raspberries2 tablespoons granulated sugar (1 ounce; 30g)1 teaspoon lemon zest";
        let result = split_concatenated_ingredient(input);
        assert_eq!(
            result,
            vec![
                "1 pound (454g) strawberries, washed, hulled, and quartered",
                "8 ounces (227g) raspberries",
                "2 tablespoons granulated sugar (1 ounce; 30g)",
                "1 teaspoon lemon zest",
            ]
        );
    }

    #[test]
    fn test_split_volume_digit() {
        let input = "1/2 teaspoon Diamond Crystal kosher salt; for table salt, use half as much by volume454g unsalted butter (1 pound; 2 cups)";
        let result = split_concatenated_ingredient(input);
        assert_eq!(
            result,
            vec![
            "1/2 teaspoon Diamond Crystal kosher salt; for table salt, use half as much by volume",
            "454g unsalted butter (1 pound; 2 cups)",
        ]
        );
    }

    #[test]
    fn test_jsonld_concatenated_ingredients_integration() {
        let html = r#"
            <!DOCTYPE html>
            <html><head>
            <script type="application/ld+json">
            {
                "@context": "http://schema.org",
                "@type": "Recipe",
                "name": "Test Cake",
                "recipeIngredient": [
                    "4 large eggs (200g)300g granulated sugar",
                    "300g granulated sugar",
                    "1 cup milk"
                ],
                "recipeInstructions": [{"@type": "HowToStep", "text": "Mix."}]
            }
            </script>
            </head><body></body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        let lines: Vec<&str> = result.ingredients.lines().collect();
        // "300g granulated sugar" appears twice: once from the split, once as a separate entry.
        // We preserve both to avoid losing intentional duplicates in other recipes.
        assert_eq!(
            lines,
            vec![
                "4 large eggs (200g)",
                "300g granulated sugar",
                "300g granulated sugar",
                "1 cup milk",
            ]
        );
    }

    #[test]
    fn test_extract_footnotes_from_wprm_notes() {
        let html = r#"
            <div class="wprm-recipe-notes-container">
                <ul>
                    <li>*If you only have salted butter that works fine too.</li>
                    <li>**Regular or dutch process cocoa works great in this recipe.</li>
                    <li>***Milk chocolate chips give the crackly top.</li>
                </ul>
            </div>
        "#;

        let footnotes = extract_footnotes_from_html(html).unwrap();
        assert_eq!(footnotes.len(), 3);
        assert_eq!(footnotes[0].0, "*");
        assert!(footnotes[0].1.contains("salted butter"));
        assert_eq!(footnotes[1].0, "**");
        assert!(footnotes[1].1.contains("cocoa"));
        assert_eq!(footnotes[2].0, "***");
        assert!(footnotes[2].1.contains("chocolate chips"));
    }

    #[test]
    fn test_extract_footnotes_skips_false_positives() {
        let html = r#"
            <div class="wprm-recipe-notes-container">
                <p>*Percent Daily Values are based on a 2,000 calorie diet.</p>
                <p>**This post may contain affiliate links for products.</p>
            </div>
        "#;

        let footnotes = extract_footnotes_from_html(html);
        assert!(footnotes.is_none());
    }

    #[test]
    fn test_extract_footnotes_none_without_notes_section() {
        let html = r#"
            <div class="recipe-content">
                <p>*This is just a blog paragraph with an asterisk.</p>
            </div>
        "#;

        let footnotes = extract_footnotes_from_html(html);
        assert!(footnotes.is_none());
    }

    #[test]
    fn test_jsonld_with_footnotes_populates_field() {
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Brownies",
                    "recipeIngredient": ["3/4 cup unsalted butter*", "2/3 cup cocoa powder**"],
                    "recipeInstructions": "Mix and bake."
                }
                </script>
            </head>
            <body>
                <div class="wprm-recipe-notes-container">
                    <ul>
                        <li>*Use European-style butter for best results.</li>
                        <li>**Dutch process cocoa is recommended here.</li>
                    </ul>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/brownies").unwrap();
        assert!(result.ingredients.contains("butter*"));
        let footnotes = result.footnotes.unwrap();
        assert_eq!(footnotes.len(), 2);
        assert_eq!(footnotes[0].0, "*");
        assert!(footnotes[0].1.contains("European-style butter"));
        assert_eq!(footnotes[1].0, "**");
        assert!(footnotes[1].1.contains("Dutch process"));
    }

    #[test]
    fn test_wprm_ingredient_groups_supplement_jsonld() {
        // WPRM JSON-LD provides flat ingredient list; HTML has the group structure.
        // The extraction should inject group headers from the HTML.
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Ginger Meatballs",
                    "recipeIngredient": [
                        "2 pounds ground pork",
                        "2 large eggs",
                        "1 can coconut milk",
                        "2 cups chicken stock"
                    ],
                    "recipeInstructions": "Make meatballs and broth."
                }
                </script>
            </head>
            <body>
                <div class="wprm-recipe-ingredient-group">
                    <span class="wprm-recipe-group-name">Meatballs</span>
                    <ul>
                        <li class="wprm-recipe-ingredient">2 pounds ground pork</li>
                        <li class="wprm-recipe-ingredient">2 large eggs</li>
                    </ul>
                </div>
                <div class="wprm-recipe-ingredient-group">
                    <span class="wprm-recipe-group-name">Broth</span>
                    <ul>
                        <li class="wprm-recipe-ingredient">1 can coconut milk</li>
                        <li class="wprm-recipe-ingredient">2 cups chicken stock</li>
                    </ul>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        let lines: Vec<&str> = result.ingredients.lines().collect();
        assert_eq!(lines[0], "Meatballs:");
        assert_eq!(lines[1], "2 pounds ground pork");
        assert_eq!(lines[2], "2 large eggs");
        assert_eq!(lines[3], "Broth:");
        assert_eq!(lines[4], "1 can coconut milk");
        assert_eq!(lines[5], "2 cups chicken stock");
    }

    #[test]
    fn test_wprm_ingredient_groups_no_names_skipped() {
        // When WPRM groups exist but none have names, fall through to flat extraction
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Simple Recipe",
                    "recipeIngredient": ["1 cup flour", "2 eggs"],
                    "recipeInstructions": "Mix."
                }
                </script>
            </head>
            <body>
                <div class="wprm-recipe-ingredient-group">
                    <span class="wprm-recipe-group-name"></span>
                    <ul>
                        <li class="wprm-recipe-ingredient">1 cup flour</li>
                        <li class="wprm-recipe-ingredient">2 eggs</li>
                    </ul>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        // Should use JSON-LD ingredients (no group headers injected)
        assert!(!result.ingredients.contains(':'));
        assert!(result.ingredients.contains("1 cup flour"));
    }

    #[test]
    fn test_jetpack_ingredient_groups_supplement_microdata() {
        // Jetpack uses <h5> headings inside .jetpack-recipe-ingredients to group
        // ingredients. Microdata extraction only picks up the [itemprop] items.
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <div itemscope itemtype="https://schema.org/Recipe">
                    <h3 class="jetpack-recipe-title" itemprop="name">Ginger Meatballs</h3>
                    <div class="jetpack-recipe-content">
                        <div class="jetpack-recipe-ingredients">
                            <h5>Meatballs</h5>
                            <ul>
                                <li class="jetpack-recipe-ingredient" itemprop="recipeIngredient">2 pounds ground pork</li>
                                <li class="jetpack-recipe-ingredient" itemprop="recipeIngredient">2 large eggs</li>
                            </ul>
                            <h5>Broth</h5>
                            <ul>
                                <li class="jetpack-recipe-ingredient" itemprop="recipeIngredient">1 can coconut milk</li>
                                <li class="jetpack-recipe-ingredient" itemprop="recipeIngredient">2 cups chicken stock</li>
                            </ul>
                            <h5>To serve</h5>
                            <ul>
                                <li class="jetpack-recipe-ingredient" itemprop="recipeIngredient">Steamed jasmine rice</li>
                            </ul>
                        </div>
                    </div>
                    <div itemprop="recipeInstructions">Make meatballs and broth.</div>
                </div>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://smittenkitchen.com/recipe").unwrap();
        let lines: Vec<&str> = result.ingredients.lines().collect();
        assert_eq!(lines[0], "Meatballs:");
        assert_eq!(lines[1], "2 pounds ground pork");
        assert_eq!(lines[2], "2 large eggs");
        assert_eq!(lines[3], "Broth:");
        assert_eq!(lines[4], "1 can coconut milk");
        assert_eq!(lines[5], "2 cups chicken stock");
        assert_eq!(lines[6], "To serve:");
        assert_eq!(lines[7], "Steamed jasmine rice");
    }

    #[test]
    fn test_jetpack_ingredient_groups_malformed_h5_inside_ul() {
        // Real smittenkitchen HTML: <p> wrapping a <div> (invalid), and <h5>
        // headers inside <ul> (also invalid). html5ever reparses this.
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <div itemscope itemtype="https://schema.org/Recipe">
                    <h3 class="jetpack-recipe-title" itemprop="name">Ginger Meatballs</h3>
                    <p><div class="jetpack-recipe-ingredients"><ul>
                        <h5>Meatballs</h5>
                        <li class="jetpack-recipe-ingredient" itemprop="recipeIngredient">2 pounds ground pork</li>
                        <li class="jetpack-recipe-ingredient" itemprop="recipeIngredient">2 large eggs</li>
                        <h5>Broth</h5>
                        <li class="jetpack-recipe-ingredient" itemprop="recipeIngredient">1 can coconut milk</li>
                    </ul></div></p>
                    <div itemprop="recipeInstructions">Make meatballs and broth.</div>
                </div>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://smittenkitchen.com/recipe").unwrap();
        eprintln!("Ingredients:\n{}", result.ingredients);
        assert!(
            result.ingredients.contains("Meatballs"),
            "should contain Meatballs group header"
        );
        assert!(
            result.ingredients.contains("Broth"),
            "should contain Broth group header"
        );
    }
}
