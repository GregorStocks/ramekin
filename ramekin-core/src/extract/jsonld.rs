//! JSON-LD (schema.org Recipe) extraction.

use super::*;

/// Regex to find JSON-LD script tags (case-insensitive for type attribute)
pub(super) static JSONLD_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<script[^>]*type\s*=\s*["']application/ld\+json["'][^>]*>(.*?)</script>"#)
        .expect("Invalid JSON-LD regex")
});

/// Fast JSON-LD extraction using regex to avoid DOM parsing.
/// Returns None if no valid JSON-LD recipe is found.
pub(super) fn extract_jsonld_fast(html: &str, source_url: &str) -> Option<RawRecipe> {
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

static JSONLD_SCRIPT_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("script[type='application/ld+json']").expect("JSON-LD script selector")
});

/// Extract recipe from JSON-LD script tags.
pub(super) fn extract_recipe_from_jsonld(
    document: &Html,
    source_url: &str,
) -> Result<RawRecipe, ExtractError> {
    for element in document.select(&JSONLD_SCRIPT_SELECTOR) {
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
pub(super) fn sanitize_json(json: &str) -> String {
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
pub(super) fn find_recipe_in_json(json: &serde_json::Value) -> Option<&serde_json::Value> {
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
pub(super) fn extract_recipe_data(
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

/// Extract ingredients as a newline-separated blob.
pub(super) fn extract_ingredients(recipe: &serde_json::Value) -> Result<String, ExtractError> {
    let ingredients_raw = recipe
        .get("recipeIngredient")
        .ok_or_else(|| ExtractError::MissingField("recipeIngredient".to_string()))?;

    let ingredients_array = ingredients_raw
        .as_array()
        .ok_or_else(|| ExtractError::InvalidJson("recipeIngredient is not an array".to_string()))?;

    let ingredients: Vec<String> = ingredients_array
        .iter()
        .filter_map(|v| v.as_str())
        .filter_map(sanitize_extracted_ingredient)
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
pub(super) fn extract_instructions(recipe: &serde_json::Value) -> Result<String, ExtractError> {
    let instructions_raw = recipe
        .get("recipeInstructions")
        .ok_or_else(|| ExtractError::MissingField("recipeInstructions".to_string()))?;

    match instructions_raw {
        serde_json::Value::String(s) => Ok(s.trim().to_string()),
        serde_json::Value::Array(arr) => {
            // Collect every step (and section header) into a flat list joined by blank
            // lines, so a recipe that mixes top-level HowToSteps with HowToSection groups
            // uses one consistent separator instead of single newlines within sections.
            let mut parts: Vec<String> = Vec::new();
            for item in arr {
                // Handle HowToStep objects
                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    let text = text.trim();
                    if !text.is_empty() {
                        parts.push(text.to_string());
                    }
                    continue;
                }
                // Handle plain strings
                if let Some(s) = item.as_str() {
                    let s = s.trim();
                    if !s.is_empty() {
                        parts.push(s.to_string());
                    }
                    continue;
                }
                // Handle HowToSection with itemListElement
                if let Some(items) = item.get("itemListElement").and_then(|v| v.as_array()) {
                    // Emit the section name as a colon-terminated header so the structure
                    // is preserved rather than silently flattened.
                    if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                        let name = name.trim();
                        if !name.is_empty() {
                            if name.ends_with(':') {
                                parts.push(name.to_string());
                            } else {
                                parts.push(format!("{name}:"));
                            }
                        }
                    }
                    for step in items {
                        if let Some(text) = step.get("text").and_then(|v| v.as_str()) {
                            let text = text.trim();
                            if !text.is_empty() {
                                parts.push(text.to_string());
                            }
                        }
                    }
                }
            }

            if parts.is_empty() {
                return Err(ExtractError::MissingField(
                    "recipeInstructions (empty)".to_string(),
                ));
            }

            Ok(parts.join("\n\n"))
        }
        _ => Err(ExtractError::InvalidJson(
            "recipeInstructions is not a string or array".to_string(),
        )),
    }
}

/// Extract image URLs from the recipe.
pub(super) fn extract_image_urls(recipe: &serde_json::Value) -> Vec<String> {
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

/// Extract whatever we can from JSON-LD without failing on missing required fields.
pub(super) fn extract_partial_from_jsonld(html: &str) -> PartialRecipe {
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
