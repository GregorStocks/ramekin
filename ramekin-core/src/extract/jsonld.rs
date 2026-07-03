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

#[cfg(test)]
#[allow(clippy::print_stdout, clippy::print_stderr)]
mod tests {
    use super::*;

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
    fn test_jsonld_strips_leading_checkbox_glyphs_from_ingredients() {
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Test Recipe",
                    "recipeIngredient": ["▢ 1 cup flour", "☑ 2 eggs"],
                    "recipeInstructions": "Mix it."
                }
                </script>
            </head><body></body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        assert_eq!(result.ingredients, "1 cup flour\n2 eggs");
    }

    #[test]
    fn test_jsonld_strips_checkbox_glyph_variation_selectors_from_ingredients() {
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Test Recipe",
                    "recipeIngredient": ["✔️ 1 cup flour", "☑️ 2 eggs"],
                    "recipeInstructions": "Mix it."
                }
                </script>
            </head><body></body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        assert_eq!(result.ingredients, "1 cup flour\n2 eggs");
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
    fn test_jsonld_howto_sections_use_consistent_separators_and_headers() {
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Sectioned Recipe",
                    "recipeIngredient": ["1 cup flour"],
                    "recipeInstructions": [
                        {"@type": "HowToStep", "text": "Gather all the ingredients."},
                        {
                            "@type": "HowToSection",
                            "name": "To Cook",
                            "itemListElement": [
                                {"@type": "HowToStep", "text": "Brown the chicken."},
                                {"@type": "HowToStep", "text": "Add the potatoes."}
                            ]
                        },
                        {
                            "@type": "HowToSection",
                            "name": "To Serve",
                            "itemListElement": [
                                {"@type": "HowToStep", "text": "Plate and garnish."}
                            ]
                        }
                    ]
                }
                </script>
            </head><body></body></html>
        "#;

        let recipe = extract_recipe(html, "https://example.com/sectioned").unwrap();
        assert_eq!(
            recipe.instructions,
            "Gather all the ingredients.\n\nTo Cook:\n\nBrown the chicken.\n\nAdd the potatoes.\n\nTo Serve:\n\nPlate and garnish."
        );
    }
}
