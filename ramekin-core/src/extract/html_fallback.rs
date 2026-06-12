//! Class/itemprop-based HTML fallback extraction (Dotdash, Jetpack, Serious Eats print pages).

use super::*;

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

    // Try Dotdash Meredith CMS (Serious Eats, Simply Recipes, Allrecipes, etc.)
    if let Some(result) = extract_dotdash_meredith_ingredients(document) {
        return Some(result);
    }

    None
}

/// Extract ingredients from Dotdash Meredith CMS pages (Serious Eats, Simply Recipes,
/// Allrecipes). Print pages frequently ship without JSON-LD; the recipe is rendered
/// only via `.structured-ingredients__list-item` lists. Group headers appear as
/// `.structured-ingredients__list-heading` paragraphs interleaved with the lists.
pub(super) fn extract_dotdash_meredith_ingredients(document: &Html) -> Option<String> {
    let container_selector =
        Selector::parse(".structured-ingredients").expect("structured ingredients selector");
    let combined_selector = Selector::parse(
        ".structured-ingredients__list-heading, .structured-ingredients__list-item",
    )
    .expect("structured ingredients heading/item selector");

    let mut lines: Vec<String> = Vec::new();
    for container in document.select(&container_selector) {
        for el in container.select(&combined_selector) {
            let class_attr = el.value().attr("class").unwrap_or("");
            if class_attr.contains("structured-ingredients__list-heading") {
                let text = el.text().collect::<String>().trim().to_string();
                if text.is_empty() {
                    continue;
                }
                if text.ends_with(':') {
                    lines.push(text);
                } else {
                    lines.push(format!("{}:", text));
                }
            } else {
                let raw_text = dotdash_ingredient_item_text(&el);
                if let Some(text) = sanitize_extracted_ingredient(&raw_text) {
                    lines.push(text);
                }
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

/// Get the visible text of a Dotdash Meredith ingredient list-item, working
/// around a CMS quirk: when an ingredient like "1 baguette" has no real unit,
/// the page renders both `data-ingredient-unit` and `data-ingredient-name`
/// spans with identical text ("1 baguette baguette"). When that pattern shows
/// up, drop the duplicated word once.
pub(super) fn dotdash_ingredient_item_text(li: &ElementRef<'_>) -> String {
    let raw_text = li.text().collect::<String>();
    let mut result = raw_text.split_whitespace().collect::<Vec<_>>().join(" ");

    let unit_selector =
        Selector::parse("[data-ingredient-unit]").expect("data-ingredient-unit selector");
    let name_selector =
        Selector::parse("[data-ingredient-name]").expect("data-ingredient-name selector");

    let unit_texts: Vec<String> = li
        .select(&unit_selector)
        .map(|el| {
            el.text()
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.is_empty())
        .collect();
    let name_texts: Vec<String> = li
        .select(&name_selector)
        .map(|el| {
            el.text()
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.is_empty())
        .collect();

    for word in &unit_texts {
        if name_texts.iter().any(|n| n == word) {
            let dup = format!("{} {}", word, word);
            if let Some(pos) = result.find(&dup) {
                result.replace_range(pos..pos + dup.len(), word);
            }
        }
    }
    result
}

/// Extract Jetpack recipe ingredients with group headers.
///
/// Jetpack structures ingredients inside a `.jetpack-recipe-ingredients` container
/// with `<h5>` (or other heading) elements as group headers interleaved with
/// `.jetpack-recipe-ingredient` list items. Microdata extraction only picks up
/// the `[itemprop]` items, losing the headings. This function walks the container's
/// children to recover the group structure.
pub(super) fn extract_jetpack_ingredients_with_groups(document: &Html) -> Option<String> {
    let container_selector =
        Selector::parse(".jetpack-recipe-ingredients").expect("jetpack ingredients selector");
    let container = document.select(&container_selector).next()?;

    let heading_selector = Selector::parse("h1, h2, h3, h4, h5, h6").expect("heading selector");

    // Check if there are any headings inside the container
    let has_headings = container.select(&heading_selector).next().is_some();
    if !has_headings {
        return None;
    }

    // Walk all descendant elements in document order, emitting headings and
    // ingredient items as we encounter them.
    let mut lines: Vec<String> = Vec::new();
    let all_selector = Selector::parse("h1, h2, h3, h4, h5, h6, .jetpack-recipe-ingredient")
        .expect("jetpack heading/ingredient selector");
    for el in container.select(&all_selector) {
        let tag = el.value().name();
        let raw_text = el.text().collect::<String>();
        let text = raw_text.trim().to_string();
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
        } else if let Some(text) = sanitize_extracted_ingredient(&raw_text) {
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
pub(super) fn extract_ingredient_items_from_selector(
    document: &Html,
    selector_str: &str,
) -> Option<String> {
    let selector = Selector::parse(selector_str).expect("ingredient items selector");
    let items: Vec<String> = document
        .select(&selector)
        .filter_map(|el| sanitize_extracted_ingredient(&el.text().collect::<String>()))
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
pub(super) fn extract_ingredients_from_div(document: &Html) -> Option<String> {
    let selector = Selector::parse("div.ingredients").expect("div.ingredients selector");
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
            if let Some(text) = sanitize_extracted_ingredient(&text) {
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
pub(super) fn extract_instructions_from_html_classes(
    document: &Html,
    recipe_title: Option<&str>,
) -> Option<String> {
    let selectors = [
        ".jetpack-recipe-directions",
        "div.instructions",
        ".recipe-instructions",
        ".e-instructions",
        ".recipe-directions",
    ];

    for selector_str in selectors {
        let selector = Selector::parse(selector_str).expect("instructions fallback selector");

        let steps: Vec<String> = document
            .select(&selector)
            .map(|el| el.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if !steps.is_empty() {
            return Some(steps.join("\n\n"));
        }
    }

    if let Some(result) = extract_dotdash_meredith_instructions(document) {
        return Some(result);
    }

    extract_wprm_instructions(document, recipe_title.unwrap_or_default())
}

/// Extract instructions from Dotdash Meredith CMS pages (Serious Eats, Simply Recipes,
/// Allrecipes). The directions section uses `.section--instructions` containing
/// `.structured-project__steps` with each step as `<li class="mntl-sc-block-group--LI">`
/// holding one or more `<p class="mntl-sc-block-html">` paragraphs.
pub(super) fn extract_dotdash_meredith_instructions(document: &Html) -> Option<String> {
    let li_selector = Selector::parse(".structured-project__steps li.mntl-sc-block-group--LI")
        .expect("dotdash steps selector");
    let p_selector =
        Selector::parse("p.mntl-sc-block-html").expect("dotdash step paragraph selector");

    let mut steps: Vec<String> = Vec::new();
    for li in document.select(&li_selector) {
        let parts: Vec<String> = li
            .select(&p_selector)
            .map(|el| {
                el.text()
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|s| !s.is_empty())
            .collect();
        if !parts.is_empty() {
            steps.push(parts.join(" "));
        }
    }

    if steps.is_empty() {
        None
    } else {
        Some(steps.join("\n\n"))
    }
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
pub(super) static JETPACK_DIRECTIONS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<div[^>]*class="[^"]*jetpack-recipe-directions[^"]*"[^>]*>.*?</div>\s*(?:<br\s*/?>)?\s*(.*?)</div>\s*</div>"#,
    )
    .expect("Invalid Jetpack directions regex")
});

pub(super) fn extract_instructions_from_raw_html(html: &str) -> Option<String> {
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
pub(super) fn html_to_paragraphs(html: &str) -> Vec<String> {
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

#[cfg(test)]
#[allow(clippy::print_stdout, clippy::print_stderr)]
mod tests {
    use super::*;

    #[test]
    fn test_html_fallback_seriouseats_print_page() {
        // Serious Eats `?print` pages ship with no JSON-LD or microdata at all —
        // just the rendered Dotdash Meredith CMS HTML. Title comes from
        // h1.heading__title, ingredients from .structured-ingredients with
        // .structured-ingredients__list-heading group headers interleaved between
        // .structured-ingredients__list-item entries, and instructions from
        // .structured-project__steps li > p.mntl-sc-block-html.
        let html = r#"
            <!doctype html>
            <html>
            <body>
                <h1 class="heading__title">Toad in the Hole</h1>
                <section class="comp section--ingredients section">
                    <div class="comp structured-ingredients">
                        <p class="structured-ingredients__list-heading">For the Yorkshire Pudding Batter:</p>
                        <ul class="structured-ingredients__list">
                            <li class="structured-ingredients__list-item"><p>3 large eggs</p></li>
                            <li class="structured-ingredients__list-item"><p>4 ounces all-purpose flour</p></li>
                        </ul>
                        <p class="structured-ingredients__list-heading">For the Red Onion Gravy</p>
                        <ul class="structured-ingredients__list">
                            <li class="structured-ingredients__list-item"><p>2 tablespoons beef drippings</p></li>
                            <li class="structured-ingredients__list-item"><p>1 large red onion, thinly sliced</p></li>
                        </ul>
                    </div>
                </section>
                <section class="comp section--instructions section">
                    <div class="comp structured-project__steps">
                        <ol class="comp mntl-sc-block-group--OL">
                            <li class="comp mntl-sc-block-group--LI">
                                <p class="comp mntl-sc-block-html"><strong>For the Batter:</strong> Whisk eggs, flour, and milk together. Let rest for 30 minutes.</p>
                            </li>
                            <li class="comp mntl-sc-block-group--LI">
                                <p class="comp mntl-sc-block-html"><strong>For the Gravy:</strong> Melt drippings over medium-low heat. Add onions and cook until lightly caramelized.</p>
                            </li>
                            <li class="comp mntl-sc-block-group--LI">
                                <p class="comp mntl-sc-block-html">Slice into wedges and smother each portion in onion gravy.</p>
                            </li>
                        </ol>
                    </div>
                </section>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://www.seriouseats.com/toad-in-the-hole").unwrap();
        assert_eq!(result.title, "Toad in the Hole");
        let ingredient_lines: Vec<&str> = result.ingredients.lines().collect();
        assert_eq!(ingredient_lines[0], "For the Yorkshire Pudding Batter:");
        assert_eq!(ingredient_lines[1], "3 large eggs");
        assert_eq!(ingredient_lines[2], "4 ounces all-purpose flour");
        assert_eq!(ingredient_lines[3], "For the Red Onion Gravy:");
        assert_eq!(ingredient_lines[4], "2 tablespoons beef drippings");
        assert_eq!(ingredient_lines[5], "1 large red onion, thinly sliced");
        assert!(result.instructions.contains("For the Batter:"));
        assert!(result.instructions.contains("Whisk eggs, flour, and milk"));
        assert!(result.instructions.contains("For the Gravy:"));
        assert!(result.instructions.contains("Slice into wedges"));
    }

    #[test]
    fn test_html_fallback_seriouseats_dedupes_unit_name_quirk() {
        // Dotdash Meredith CMS quirk: when an ingredient like "1 baguette"
        // has no real unit, the page renders both `data-ingredient-unit` and
        // `data-ingredient-name` spans with the same word, producing
        // "1 baguette baguette". Strip the duplicate.
        let html = r#"
            <!doctype html>
            <html>
            <body>
                <h1 class="heading__title">Crusty Bread</h1>
                <div class="comp structured-ingredients">
                    <ul class="structured-ingredients__list">
                        <li class="structured-ingredients__list-item">
                            <p>
                                <span data-ingredient-quantity="true">1</span>
                                <span data-ingredient-unit="true">baguette</span>
                                <span data-ingredient-name="true">baguette</span>
                            </p>
                        </li>
                    </ul>
                </div>
                <div class="comp structured-project__steps">
                    <ol>
                        <li class="comp mntl-sc-block-group--LI">
                            <p class="comp mntl-sc-block-html">Slice the baguette and serve.</p>
                        </li>
                    </ol>
                </div>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://www.seriouseats.com/baguette").unwrap();
        let ingredient_lines: Vec<&str> = result.ingredients.lines().collect();
        assert_eq!(ingredient_lines, vec!["1 baguette"]);
    }

    #[test]
    fn test_html_fallback_seriouseats_no_groups() {
        // Print page with a single ingredient list and no group headings.
        let html = r#"
            <!doctype html>
            <html>
            <body>
                <h1 class="heading__title">Simple Vinaigrette</h1>
                <div class="comp structured-ingredients">
                    <ul class="structured-ingredients__list">
                        <li class="structured-ingredients__list-item"><p>3 tablespoons olive oil</p></li>
                        <li class="structured-ingredients__list-item"><p>1 tablespoon vinegar</p></li>
                    </ul>
                </div>
                <div class="comp structured-project__steps">
                    <ol>
                        <li class="comp mntl-sc-block-group--LI">
                            <p class="comp mntl-sc-block-html">Whisk oil and vinegar together until emulsified.</p>
                        </li>
                    </ol>
                </div>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://www.seriouseats.com/vinaigrette").unwrap();
        assert_eq!(result.title, "Simple Vinaigrette");
        assert!(result.ingredients.contains("3 tablespoons olive oil"));
        assert!(result.ingredients.contains("1 tablespoon vinegar"));
        // No spurious group header should appear.
        assert!(!result.ingredients.contains(":"));
        assert!(result.instructions.contains("Whisk oil and vinegar"));
    }

    #[test]
    fn test_dotdash_visible_ingredients_supplement_jsonld() {
        // Dotdash Meredith (Serious Eats) JSON-LD simplifies combined ingredient
        // rows, dropping quantities the visible page keeps. The visible
        // .structured-ingredients rows should win.
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Croquetas de Jamón",
                    "recipeIngredient": [
                        "2 cups (473ml) whole milk",
                        "1 cup all-purpose flour, for dredging"
                    ],
                    "recipeInstructions": "Stir in flour, then dredge and fry."
                }
                </script>
            </head>
            <body>
                <div class="comp structured-ingredients">
                    <ul class="structured-ingredients__list">
                        <li class="structured-ingredients__list-item"><p>2 cups (473 ml) whole milk</p></li>
                        <li class="structured-ingredients__list-item"><p>1/2 cup plus 2 tablespoons all-purpose flour (80 g), plus 1 cup all-purpose flour (for dredging), divided</p></li>
                    </ul>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://www.seriouseats.com/croquetas").unwrap();
        let lines: Vec<&str> = result.ingredients.lines().collect();
        assert_eq!(
            lines,
            vec![
                "2 cups (473 ml) whole milk",
                "1/2 cup plus 2 tablespoons all-purpose flour (80 g), plus 1 cup all-purpose flour (for dredging), divided",
            ]
        );
    }

    #[test]
    fn test_dotdash_normalized_visible_ingredients_keep_jsonld() {
        // Some Dotdash pages render nutrition-database normalized rows instead
        // of the author's text ("454 g pork breakfast sausage" for "1 pound
        // (454g) pork breakfast sausage, casings removed"). Those rows sit
        // entirely inside data-ingredient-* spans with no free text outside;
        // keep the JSON-LD version, which has the author's rows.
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Biscuits and Gravy",
                    "recipeIngredient": [
                        "1 pound (454g) pork breakfast sausage, casings removed",
                        "Freshly ground black pepper"
                    ],
                    "recipeInstructions": "Brown the sausage and make the gravy."
                }
                </script>
            </head>
            <body>
                <div class="comp structured-ingredients">
                    <ul class="structured-ingredients__list">
                        <li class="structured-ingredients__list-item">
                            <p><span data-ingredient-quantity="true">454</span> <span data-ingredient-unit="true">g</span> <span data-ingredient-name="true">pork breakfast sausage</span></p>
                        </li>
                        <li class="structured-ingredients__list-item">
                            <p><span data-ingredient-quantity="true">1</span> <span data-ingredient-unit="true">tsp, ground</span> <span data-ingredient-name="true">ground black pepper</span></p>
                        </li>
                    </ul>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://www.seriouseats.com/biscuits").unwrap();
        let lines: Vec<&str> = result.ingredients.lines().collect();
        assert_eq!(
            lines,
            vec![
                "1 pound (454g) pork breakfast sausage, casings removed",
                "Freshly ground black pepper",
            ]
        );
    }

    #[test]
    fn test_dotdash_visible_ingredients_fewer_rows_keeps_jsonld() {
        // If the rendered page shows fewer ingredient rows than the structured
        // data (e.g. a partially rendered list), keep the JSON-LD version.
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Croquetas de Jamón",
                    "recipeIngredient": [
                        "2 cups (473ml) whole milk",
                        "1 cup all-purpose flour, for dredging"
                    ],
                    "recipeInstructions": "Stir in flour, then dredge and fry."
                }
                </script>
            </head>
            <body>
                <div class="comp structured-ingredients">
                    <ul class="structured-ingredients__list">
                        <li class="structured-ingredients__list-item"><p>2 cups (473 ml) whole milk</p></li>
                    </ul>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://www.seriouseats.com/croquetas").unwrap();
        let lines: Vec<&str> = result.ingredients.lines().collect();
        assert_eq!(
            lines,
            vec![
                "2 cups (473ml) whole milk",
                "1 cup all-purpose flour, for dredging",
            ]
        );
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
