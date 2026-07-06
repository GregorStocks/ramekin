//! Microdata (itemprop/itemscope) extraction.

use super::*;

/// The Recipe container element; both http and https schema.org URLs.
static RECIPE_CONTAINER_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(
        r#"[itemtype="http://schema.org/Recipe"], [itemtype="https://schema.org/Recipe"]"#,
    )
    .expect("recipe container selector")
});

static INGREDIENT_ITEMPROP_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(r#"[itemprop="recipeIngredient"], [itemprop="ingredients"]"#)
        .expect("ingredient itemprop selector")
});

static INSTRUCTIONS_ITEMPROP_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(r#"[itemprop="recipeInstructions"], [itemprop="instructions"]"#)
        .expect("instructions itemprop selector")
});

/// Extract recipe from schema.org microdata markup.
/// This is a fallback for sites that don't use JSON-LD but have microdata attributes.
pub(super) fn extract_recipe_from_microdata(
    document: &Html,
    source_url: &str,
) -> Result<RawRecipe, ExtractError> {
    let recipe_element = document
        .select(&RECIPE_CONTAINER_SELECTOR)
        .next()
        .ok_or(ExtractError::NoRecipe)?;

    // Extract title from itemprop="name"
    let title = extract_microdata_text(&recipe_element, &NAME_ITEMPROP_SELECTOR)
        .ok_or_else(|| ExtractError::MissingField("name".to_string()))?;
    let title = decode_html_entities(&title);

    // Extract description (optional)
    let description = extract_microdata_text(&recipe_element, &DESCRIPTION_ITEMPROP_SELECTOR)
        .map(|s| decode_html_entities(&s));

    // Extract ingredients
    let ingredients: Vec<String> = recipe_element
        .select(&INGREDIENT_ITEMPROP_SELECTOR)
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
    let servings = extract_microdata_text(&recipe_element, &YIELD_ITEMPROP_SELECTOR);

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

static NAME_ITEMPROP_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"[itemprop="name"]"#).expect("itemprop name selector"));

static DESCRIPTION_ITEMPROP_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(r#"[itemprop="description"]"#).expect("itemprop description selector")
});

static YIELD_ITEMPROP_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(r#"[itemprop="recipeYield"]"#).expect("itemprop recipeYield selector")
});

/// Extract text content from an element matching the given itemprop selector.
pub(super) fn extract_microdata_text(
    element: &scraper::ElementRef,
    selector: &Selector,
) -> Option<String> {
    element.select(selector).next().map(|el| {
        // Check for content attribute first (common for meta tags)
        if let Some(content) = el.value().attr("content") {
            content.trim().to_string()
        } else {
            el.text().collect::<String>().trim().to_string()
        }
    })
}

static INSTRUCTION_STEP_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(
        r#"[itemprop="recipeInstructions"], [itemprop="instructions"], [itemtype*="HowToStep"]"#,
    )
    .expect("instruction step selector")
});

/// Text property inside HowToStep
static TEXT_ITEMPROP_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"[itemprop="text"]"#).expect("itemprop text selector"));

/// Fallback: h-recipe microformat classes (used by Jetpack and others).
static INSTRUCTION_CLASS_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(
        r#".e-instructions, .instructions, .recipe-instructions, .jetpack-recipe-directions, .recipe-directions"#,
    )
    .expect("instruction class selector")
});

/// Extract instructions from microdata.
pub(super) fn extract_microdata_instructions(
    recipe_element: &scraper::ElementRef,
) -> Result<String, ExtractError> {
    // Try to find instruction elements using schema.org microdata
    let steps: Vec<String> = recipe_element
        .select(&INSTRUCTION_STEP_SELECTOR)
        .map(|el| {
            if let Some(text_el) = el.select(&TEXT_ITEMPROP_SELECTOR).next() {
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
    let instructions: Vec<String> = recipe_element
        .select(&INSTRUCTION_CLASS_SELECTOR)
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

static IMAGE_ITEMPROP_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"[itemprop="image"]"#).expect("itemprop image selector"));

/// Extract image URLs from microdata.
pub(super) fn extract_microdata_images(recipe_element: &scraper::ElementRef) -> Vec<String> {
    recipe_element
        .select(&IMAGE_ITEMPROP_SELECTOR)
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
    let recipe_element = match document.select(&RECIPE_CONTAINER_SELECTOR).next() {
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

    let title = extract_microdata_text(&recipe_element, &NAME_ITEMPROP_SELECTOR);
    let description = extract_microdata_text(&recipe_element, &DESCRIPTION_ITEMPROP_SELECTOR);

    let ingredients_vec: Vec<String> = recipe_element
        .select(&INGREDIENT_ITEMPROP_SELECTOR)
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
    let servings = extract_microdata_text(&recipe_element, &YIELD_ITEMPROP_SELECTOR);

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
    let ingredients: Vec<String> = document
        .select(&INGREDIENT_ITEMPROP_SELECTOR)
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
    let steps: Vec<String> = document
        .select(&INSTRUCTIONS_ITEMPROP_SELECTOR)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if steps.is_empty() {
        None
    } else {
        Some(steps.join("\n\n"))
    }
}
