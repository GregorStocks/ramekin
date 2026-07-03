use std::sync::LazyLock;

use regex::Regex;

use crate::error::ExtractError;
use crate::ingredient_parser::detect_section_header;
use crate::types::{ExtractRecipeOutput, ExtractionAttempt, ExtractionMethod, RawRecipe};
use scraper::{ElementRef, Html, Selector};

mod blog;
mod footnotes;
mod html_fallback;
mod images;
mod jsonld;
mod microdata;
mod substack;
mod wprm;

use blog::*;
use footnotes::*;
use html_fallback::*;
use images::*;
use jsonld::*;
use microdata::*;
use substack::*;
use wprm::*;

pub use footnotes::extract_footnotes_from_html;

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
        // Structured data can miss richer HTML-only structure or include
        // polluted instruction text; supplement from the rendered recipe card.
        if should_parse_html_for_supplements(html) {
            let document = Html::parse_document(html);
            supplement_recipe_from_html(&mut recipe, &document);
        }
        return Ok(recipe);
    }

    // Slow path: full DOM parsing for malformed HTML or microdata-only sites
    let document = Html::parse_document(html);

    // Try JSON-LD via DOM (handles edge cases regex might miss)
    if let Ok(mut recipe) = extract_recipe_from_jsonld(&document, source_url) {
        supplement_recipe_from_html(&mut recipe, &document);
        return Ok(recipe);
    }

    // Fall back to microdata
    if let Ok(mut recipe) = extract_recipe_from_microdata(&document, source_url) {
        supplement_recipe_from_html(&mut recipe, &document);
        return Ok(recipe);
    }

    // Last resort: supplement partial structured data with HTML fallbacks
    extract_recipe_with_html_fallback(html, &document, source_url)
}

fn should_parse_html_for_supplements(html: &str) -> bool {
    html.contains("wprm-recipe-group-name")
        || html.contains("jetpack-recipe-ingredients")
        || html.contains("wprm-recipe-instruction")
        || html.contains("structured-ingredients__list-item")
}

/// Try to replace flat ingredients or polluted instructions with cleaner HTML-derived content.
fn supplement_recipe_from_html(recipe: &mut RawRecipe, document: &Html) {
    supplement_ingredient_groups(recipe, document);
    supplement_dotdash_ingredients(recipe, document);
    supplement_instructions(recipe, document);
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

/// Prefer the visible Dotdash Meredith (Serious Eats, Simply Recipes, Allrecipes)
/// ingredient rows over the structured data. Dotdash JSON-LD simplifies combined
/// rows, dropping quantities the rendered page keeps — e.g. the visible
/// "1/2 cup plus 2 tablespoons all-purpose flour (80 g), plus 1 cup all-purpose
/// flour (for dredging), divided" becomes just "1 cup all-purpose flour, for
/// dredging". Only replace when the visible list covers at least as many
/// ingredients as the structured data, so a partially rendered page can't drop
/// rows that JSON-LD has.
fn supplement_dotdash_ingredients(recipe: &mut RawRecipe, document: &Html) {
    if dotdash_ingredients_look_normalized(document) {
        return;
    }
    let Some(html_ingredients) = extract_dotdash_meredith_ingredients(document) else {
        return;
    };
    let html_count = html_ingredients
        .lines()
        .filter(|line| !line.ends_with(':'))
        .count();
    let structured_count = recipe
        .ingredients
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    if html_count >= structured_count {
        recipe.ingredients = html_ingredients;
    }
}

/// Detect Dotdash pages whose visible ingredient rows are nutrition-database
/// normalized (e.g. "454 g pork breakfast sausage", "1 tsp, ground ground black
/// pepper") rather than the author's text. On those pages every row's text sits
/// entirely inside data-ingredient-* spans, with no free text (parentheticals,
/// "divided", "see notes") outside them; the author's rows live only in the
/// JSON-LD, so the visible rows must not replace it.
static STRUCTURED_INGREDIENT_ITEM_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(".structured-ingredients__list-item")
        .expect("structured ingredients list item selector")
});

static DATA_INGREDIENT_SPAN_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("[data-ingredient-quantity], [data-ingredient-unit], [data-ingredient-name]")
        .expect("data-ingredient span selector")
});

fn dotdash_ingredients_look_normalized(document: &Html) -> bool {
    // Whitespace is stripped entirely (not normalized to single spaces) so the
    // comparison is insensitive to how whitespace falls between text nodes.
    let mut saw_item = false;
    for item in document.select(&STRUCTURED_INGREDIENT_ITEM_SELECTOR) {
        saw_item = true;
        let full_text: String = item.text().collect::<String>().split_whitespace().collect();
        let spanned_text: String = item
            .select(&DATA_INGREDIENT_SPAN_SELECTOR)
            .flat_map(|el| el.text())
            .collect::<String>()
            .split_whitespace()
            .collect();
        if full_text != spanned_text {
            return false;
        }
    }
    saw_item
}

/// Prefer cleaned HTML instructions when the rendered recipe card is more accurate than
/// the structured data. WPRM can flatten hidden sticky-note text into JSON-LD.
fn supplement_instructions(recipe: &mut RawRecipe, document: &Html) {
    if let Some(instructions) = extract_wprm_instructions(document, &recipe.title) {
        recipe.instructions = instructions;
    }
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
        if should_parse_html_for_supplements(html) {
            let document = Html::parse_document(html);
            supplement_recipe_from_html(&mut recipe, &document);
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
        supplement_recipe_from_html(&mut recipe, &document);
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
        supplement_recipe_from_html(&mut recipe, &document);
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

fn sanitize_extracted_ingredient(text: &str) -> Option<String> {
    let decoded = decode_html_entities(text.trim());
    let mut sanitized = decoded.trim();

    loop {
        let mut chars = sanitized.chars();
        let Some(first) = chars.next() else {
            break;
        };

        if !matches!(first, '▢' | '☐' | '☑' | '☒' | '✓' | '✔') {
            break;
        }

        sanitized = chars.as_str().trim_start_matches(['\u{fe0e}', '\u{fe0f}']);
        sanitized = sanitized.trim_start();
    }

    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized.to_string())
    }
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

/// Regex to strip HTML tags for extracting text from raw HTML fragments.
static HTML_TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<[^>]+>").expect("Invalid HTML tag regex"));

static LI_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("li").expect("li selector"));

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

/// Partial recipe data extracted leniently (missing required fields are None, not errors).
struct PartialRecipe {
    title: Option<String>,
    description: Option<String>,
    ingredients: Option<String>,
    instructions: Option<String>,
    image_urls: Vec<String>,
    servings: Option<String>,
}

static TITLE_FALLBACK_SELECTORS: LazyLock<[Selector; 5]> = LazyLock::new(|| {
    [
        ".jetpack-recipe-title",
        ".wprm-recipe-name",
        "h1.entry-title",
        "h2.entry-title",
        "h1.heading__title",
    ]
    .map(|s| Selector::parse(s).expect("title fallback selector"))
});

static TITLE_TAG_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("title").expect("title selector"));

/// Extract a recipe title from common HTML elements when structured data lacks a name.
fn extract_title_from_html(document: &Html) -> Option<String> {
    for selector in TITLE_FALLBACK_SELECTORS.iter() {
        if let Some(el) = document.select(selector).next() {
            let text = el.text().collect::<String>();
            let text = text.trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }

    // Last resort: <title> tag, stripped of site name suffix
    let title_el = document.select(&TITLE_TAG_SELECTOR).next()?;
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
#[allow(clippy::print_stdout, clippy::print_stderr)]
mod tests {
    use super::*;

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
}
