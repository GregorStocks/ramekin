use crate::error::ExtractError;
use crate::types::RawRecipe;
use scraper::{Html, Selector};

/// Extract a recipe from HTML containing JSON-LD structured data.
/// Falls back to microdata extraction if JSON-LD fails.
pub fn extract_recipe(html: &str, source_url: &str) -> Result<RawRecipe, ExtractError> {
    let document = Html::parse_document(html);

    // First try JSON-LD extraction
    if let Ok(recipe) = extract_recipe_from_jsonld(&document, source_url) {
        return Ok(recipe);
    }

    // Fall back to microdata extraction
    if let Ok(recipe) = extract_recipe_from_microdata(&document, source_url) {
        return Ok(recipe);
    }

    Err(ExtractError::NoRecipe)
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
            return extract_recipe_data(recipe, source_url);
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

    Ok(RawRecipe {
        title,
        description,
        ingredients,
        instructions,
        image_urls,
        source_url: source_url.to_string(),
        source_name,
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
    let image_urls = extract_microdata_images(&recipe_element);

    let source_name = extract_source_name(source_url);

    Ok(RawRecipe {
        title,
        description,
        ingredients: ingredients.join("\n"),
        instructions,
        image_urls,
        source_url: source_url.to_string(),
        source_name,
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
