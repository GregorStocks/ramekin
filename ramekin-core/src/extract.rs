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

/// Extract a recipe from HTML containing JSON-LD structured data.
/// Falls back to microdata extraction if JSON-LD fails, then tries
/// supplementing partial structured data with HTML class-based extraction.
///
/// Uses a fast regex-based path for JSON-LD to avoid full DOM parsing.
pub fn extract_recipe(html: &str, source_url: &str) -> Result<RawRecipe, ExtractError> {
    // Fast path: extract JSON-LD using regex (avoids DOM parsing)
    if let Some(recipe) = extract_jsonld_fast(html, source_url) {
        return Ok(recipe);
    }

    // Slow path: full DOM parsing for malformed HTML or microdata-only sites
    let document = Html::parse_document(html);

    // Try JSON-LD via DOM (handles edge cases regex might miss)
    if let Ok(recipe) = extract_recipe_from_jsonld(&document, source_url) {
        return Ok(recipe);
    }

    // Fall back to microdata
    if let Ok(recipe) = extract_recipe_from_microdata(&document, source_url) {
        return Ok(recipe);
    }

    // Last resort: supplement partial structured data with HTML fallbacks
    extract_recipe_with_html_fallback(html, &document, source_url)
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
    if let Some(recipe) = extract_jsonld_fast(html, source_url) {
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
    if let Ok(recipe) = jsonld_result {
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
    if let Ok(recipe) = microdata_result {
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
        .ok_or_else(|| ExtractError::MissingField("name".to_string()))?
        .to_string();

    let description = recipe
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

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
    })
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
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty()) // Filter out empty/whitespace-only strings
        .collect();

    if ingredients.is_empty() {
        return Err(ExtractError::MissingField(
            "recipeIngredient (empty)".to_string(),
        ));
    }

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

    // Extract description (optional)
    let description = extract_microdata_text(&recipe_element, "description");

    // Extract ingredients
    let ingredient_selector =
        Selector::parse(r#"[itemprop="recipeIngredient"], [itemprop="ingredients"]"#)
            .expect("Invalid selector");
    let ingredients: Vec<String> = recipe_element
        .select(&ingredient_selector)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

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

    Ok(RawRecipe {
        title,
        description,
        ingredients: ingredients.join("\n"),
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

/// Regex to strip HTML tags for extracting text from raw HTML fragments.
static HTML_TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<[^>]+>").expect("Invalid HTML tag regex"));

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

    // For any still-missing required fields, try HTML fallbacks
    let title = title
        .or_else(|| extract_title_from_html(document))
        .ok_or_else(|| ExtractError::MissingField("name".to_string()))?;

    let ingredients = ingredients
        .or_else(|| extract_ingredients_from_html_classes(document))
        .or_else(|| extract_ingredients_from_itemprop_unscoped(document))
        .ok_or_else(|| ExtractError::MissingField("recipeIngredient (empty)".to_string()))?;

    let instructions = instructions
        .or_else(|| extract_instructions_from_html_classes(document))
        .or_else(|| extract_instructions_from_itemprop_unscoped(document))
        .or_else(|| extract_instructions_from_raw_html(html))
        .ok_or_else(|| ExtractError::MissingField("recipeInstructions (empty)".to_string()))?;

    let mut image_urls = image_urls;
    if image_urls.is_empty() {
        if let Some(og_image) = extract_og_image(document) {
            image_urls.push(og_image);
        }
    }

    let source_name = extract_source_name(source_url);

    Ok(RawRecipe {
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
    })
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
    // Try Jetpack recipe ingredient list items
    if let Some(result) = extract_from_selector_items(document, ".jetpack-recipe-ingredient") {
        return Some(result);
    }

    // Try div.ingredients with <br>-separated content (mybakingaddiction old format)
    if let Some(result) = extract_ingredients_from_div(document) {
        return Some(result);
    }

    // Try WP Recipe Maker
    if let Some(result) = extract_from_selector_items(document, ".wprm-recipe-ingredient") {
        return Some(result);
    }

    // Try Tasty Recipes
    if let Some(result) = extract_from_selector_items(document, ".tasty-recipe-ingredients li") {
        return Some(result);
    }

    None
}

/// Extract text items from a CSS selector, joining with newlines.
fn extract_from_selector_items(document: &Html, selector_str: &str) -> Option<String> {
    let selector = Selector::parse(selector_str).ok()?;
    let items: Vec<String> = document
        .select(&selector)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

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
}
