//! Microdata (itemprop/itemscope) extraction.

use super::*;

/// Extract recipe from schema.org microdata markup.
/// This is a fallback for sites that don't use JSON-LD but have microdata attributes.
pub(super) fn extract_recipe_from_microdata(
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
        .filter_map(|el| sanitize_extracted_ingredient(&el.text().collect::<String>()))
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
pub(super) fn extract_microdata_text(element: &scraper::ElementRef, prop: &str) -> Option<String> {
    let selector =
        Selector::parse(&format!(r#"[itemprop="{}"]"#, prop)).expect("itemprop selector");
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
pub(super) fn extract_microdata_instructions(
    recipe_element: &scraper::ElementRef,
) -> Result<String, ExtractError> {
    // Try to find instruction elements using schema.org microdata
    let step_selector = Selector::parse(
        r#"[itemprop="recipeInstructions"], [itemprop="instructions"], [itemtype*="HowToStep"]"#,
    )
    .expect("Invalid selector");

    // Text property inside HowToStep
    let text_selector = Selector::parse(r#"[itemprop="text"]"#).expect("itemprop text selector");
    let steps: Vec<String> = recipe_element
        .select(&step_selector)
        .map(|el| {
            if let Some(text_el) = el.select(&text_selector).next() {
                return text_el.text().collect::<String>().trim().to_string();
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
pub(super) fn extract_microdata_images(recipe_element: &scraper::ElementRef) -> Vec<String> {
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

/// Extract whatever we can from microdata without failing on missing required fields.
pub(super) fn extract_partial_from_microdata(document: &Html) -> PartialRecipe {
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
pub(super) fn extract_ingredients_from_itemprop_unscoped(document: &Html) -> Option<String> {
    let selector = Selector::parse(r#"[itemprop="recipeIngredient"], [itemprop="ingredients"]"#)
        .expect("Invalid selector");
    let ingredients: Vec<String> = document
        .select(&selector)
        .filter_map(|el| sanitize_extracted_ingredient(&el.text().collect::<String>()))
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
pub(super) fn extract_instructions_from_itemprop_unscoped(document: &Html) -> Option<String> {
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

#[cfg(test)]
#[allow(clippy::print_stdout, clippy::print_stderr)]
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

    #[test]
    fn test_microdata_strips_leading_checkbox_glyphs_from_ingredients() {
        let html = r#"
            <!DOCTYPE html>
            <html><body>
                <div itemscope itemtype="https://schema.org/Recipe">
                    <h1 itemprop="name">Test Recipe</h1>
                    <ul>
                        <li itemprop="recipeIngredient">▢ 1 cup flour</li>
                        <li itemprop="recipeIngredient">✓ 2 eggs</li>
                    </ul>
                    <div itemprop="recipeInstructions">Mix it.</div>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        assert_eq!(result.ingredients, "1 cup flour\n2 eggs");
    }

    // --- Concatenated ingredient splitting tests ---
}
