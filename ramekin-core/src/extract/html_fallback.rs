//! Class/itemprop-based HTML fallback extraction coordinator.

use super::*;

mod dotdash;
mod plugins;

pub(super) use dotdash::*;
pub(super) use plugins::*;

/// Extract a recipe by combining partial structured data with HTML class-based fallbacks.
/// This handles cases where JSON-LD or microdata has a Recipe object but with empty required
/// fields, while the actual content exists in HTML elements with common recipe plugin classes.
pub(super) fn extract_recipe_with_html_fallback(
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
        .or_else(|| extract_instructions_from_html_classes(document, title.as_deref()))
        .or_else(|| extract_instructions_from_itemprop_unscoped(document))
        .or_else(|| {
            extract_smittenkitchen_post_instructions(
                document,
                source_url,
                title.as_deref(),
                ingredients.as_deref(),
            )
        })
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

    // Site-aware fallback for virtualweberbullet.com — pages are hand-authored HTML
    // without Recipe JSON-LD or microdata, but follow a consistent layout.
    if let Some(recipe) = extract_recipe_from_virtualweberbullet(html, document, source_url) {
        return Ok(recipe);
    }

    // Last resort: try unstructured blog recipe extraction (handles older WordPress
    // posts that write recipes in plain HTML without any recipe plugin)
    if let Some(recipe) = extract_recipe_from_unstructured_blog(html, source_url) {
        return Ok(recipe);
    }

    // Substack newsletter posts: NewsArticle JSON-LD, recipe text in `<div class="body markup">`
    if let Some(recipe) = extract_recipe_from_substack(document, source_url) {
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

/// Extract ingredients from common recipe plugin HTML classes.
/// Searches the entire document (not scoped to a microdata container).
pub(super) fn extract_ingredients_from_html_classes(document: &Html) -> Option<String> {
    // Try Jetpack recipe ingredient list items (with group support)
    if let Some(result) = extract_jetpack_ingredients_with_groups(document) {
        return Some(result);
    }
    // Fallback: Jetpack without groups
    if let Some(result) =
        extract_ingredient_items_from_selector(document, &JETPACK_INGREDIENT_SELECTOR)
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
        extract_ingredient_items_from_selector(document, &wprm::WPRM_INGREDIENT_SELECTOR)
    {
        return Some(result);
    }

    // Try Tasty Recipes
    if let Some(result) =
        extract_ingredient_items_from_selector(document, &TASTY_INGREDIENT_SELECTOR)
    {
        return Some(result);
    }

    // Try Dotdash Meredith CMS (Serious Eats, Simply Recipes, Allrecipes, etc.)
    if let Some(result) = extract_dotdash_meredith_ingredients(document) {
        return Some(result);
    }

    None
}

#[cfg(test)]
#[allow(clippy::print_stdout, clippy::print_stderr)]
mod tests;
