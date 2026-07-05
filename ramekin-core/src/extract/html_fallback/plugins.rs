use super::*;

static JETPACK_INGREDIENTS_CONTAINER_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(".jetpack-recipe-ingredients").expect("jetpack ingredients selector")
});

static HEADING_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("h1, h2, h3, h4, h5, h6").expect("heading selector"));

static JETPACK_HEADING_INGREDIENT_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("h1, h2, h3, h4, h5, h6, .jetpack-recipe-ingredient")
        .expect("jetpack heading/ingredient selector")
});

/// Extract Jetpack recipe ingredients with group headers.
///
/// Jetpack structures ingredients inside a `.jetpack-recipe-ingredients` container
/// with `<h5>` (or other heading) elements as group headers interleaved with
/// `.jetpack-recipe-ingredient` list items. Microdata extraction only picks up
/// the `[itemprop]` items, losing the headings. This function walks the container's
/// children to recover the group structure.
pub(in crate::extract) fn extract_jetpack_ingredients_with_groups(
    document: &Html,
) -> Option<String> {
    let container = document
        .select(&JETPACK_INGREDIENTS_CONTAINER_SELECTOR)
        .next()?;

    // Check if there are any headings inside the container
    let has_headings = container.select(&HEADING_SELECTOR).next().is_some();
    if !has_headings {
        return None;
    }

    // Walk all descendant elements in document order, emitting headings and
    // ingredient items as we encounter them.
    let mut lines: Vec<String> = Vec::new();
    for el in container.select(&JETPACK_HEADING_INGREDIENT_SELECTOR) {
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

pub(in crate::extract::html_fallback) static JETPACK_INGREDIENT_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| {
        Selector::parse(".jetpack-recipe-ingredient").expect("jetpack ingredient selector")
    });

pub(in crate::extract::html_fallback) static TASTY_INGREDIENT_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| {
        Selector::parse(".tasty-recipe-ingredients li").expect("tasty ingredient selector")
    });

/// Extract ingredient items from a CSS selector, splitting concatenated entries and deduplicating.
pub(in crate::extract) fn extract_ingredient_items_from_selector(
    document: &Html,
    selector: &Selector,
) -> Option<String> {
    let items: Vec<String> = document
        .select(selector)
        .filter_map(|el| sanitize_extracted_ingredient(&el.text().collect::<String>()))
        .collect();
    let items = split_and_dedup_ingredients(items);

    if items.is_empty() {
        None
    } else {
        Some(items.join("\n"))
    }
}

static INGREDIENTS_DIV_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.ingredients").expect("div.ingredients selector"));

/// Splits on <br>, <br/>, <br />, </p><p>, </p>, <p>
static INGREDIENT_DIV_SPLIT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<br\s*/?>|</p>\s*<p>|</?p>").expect("Invalid ingredient div split regex")
});

/// Extract ingredients from a `<div class="ingredients">` container.
/// Handles the old WordPress recipe format where ingredients are in `<p>` tags
/// separated by `<br>` elements, with optional `<h4>` section headers.
pub(in crate::extract) fn extract_ingredients_from_div(document: &Html) -> Option<String> {
    let div = document.select(&INGREDIENTS_DIV_SELECTOR).next()?;

    // Get the inner HTML and split on <br> tags to get individual lines
    let inner_html = div.inner_html();
    let mut lines: Vec<String> = Vec::new();

    for chunk in INGREDIENT_DIV_SPLIT_REGEX.split(&inner_html) {
        // Strip remaining HTML tags and decode entities
        let text = fragment_to_text(chunk);
        if let Some(text) = sanitize_extracted_ingredient(&text) {
            lines.push(text);
        }
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

static INSTRUCTION_FALLBACK_SELECTORS: LazyLock<[Selector; 5]> = LazyLock::new(|| {
    [
        ".jetpack-recipe-directions",
        "div.instructions",
        ".recipe-instructions",
        ".e-instructions",
        ".recipe-directions",
    ]
    .map(|s| Selector::parse(s).expect("instructions fallback selector"))
});

/// Extract instructions from common recipe plugin HTML classes.
/// Searches the entire document (not scoped to a microdata container).
pub(in crate::extract) fn extract_instructions_from_html_classes(
    document: &Html,
    recipe_title: Option<&str>,
) -> Option<String> {
    for selector in INSTRUCTION_FALLBACK_SELECTORS.iter() {
        let steps: Vec<String> = document
            .select(selector)
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
pub(in crate::extract) static JETPACK_DIRECTIONS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<div[^>]*class="[^"]*jetpack-recipe-directions[^"]*"[^>]*>.*?</div>\s*(?:<br\s*/?>)?\s*(.*?)</div>\s*</div>"#,
    )
    .expect("Invalid Jetpack directions regex")
});

pub(in crate::extract) fn extract_instructions_from_raw_html(html: &str) -> Option<String> {
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

static PARAGRAPH_SPLIT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)</p>\s*<p[^>]*>|<br\s*/?\s*>\s*<br\s*/?\s*>")
        .expect("Invalid paragraph split regex")
});

/// Convert an HTML fragment into a list of plain-text paragraphs.
/// Splits on `</p><p>` and `<br><br>` boundaries, strips tags, decodes entities.
pub(in crate::extract) fn html_to_paragraphs(html: &str) -> Vec<String> {
    PARAGRAPH_SPLIT_REGEX
        .split(html)
        .map(fragment_to_text)
        .filter(|s| !s.is_empty())
        .collect()
}
