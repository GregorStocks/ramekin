//! Substack post extraction.

use super::*;

pub(super) static SUBSTACK_BODY_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.body.markup").expect("substack body markup selector"));

pub(super) static LI_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("li").expect("li selector"));

/// Recipe block discovered while scanning a Substack post body.
pub(super) struct SubstackBlock {
    title: String,
    ingredients: Vec<String>,
    instructions: Vec<String>,
}

/// Extract a recipe from a Substack post's `<div class="body markup">` body.
///
/// Substack posts embed recipes as a heading (`<h2>..<h6>`) followed by an
/// ingredient list (`<ul>` of `<li><p>...</p></li>`, or plain `<p>` lines on
/// older posts), an `INSTRUCTIONS`/`DIRECTIONS` marker `<p>`, and an
/// instruction list (`<ol>` or plain `<p>` paragraphs). Posts can contain
/// multiple recipe blocks (e.g. a sub-recipe plus the main drink); blocks
/// join via section-header lines like the unstructured-blog path.
///
/// Returns None for paywalled posts: the body markup is present but stops at
/// the teaser, so no block accumulates an `INSTRUCTIONS` marker.
pub(super) fn extract_recipe_from_substack(document: &Html, source_url: &str) -> Option<RawRecipe> {
    let body = document.select(&SUBSTACK_BODY_SELECTOR).next()?;
    let blocks = scan_substack_blocks(body);
    if blocks.is_empty() {
        return None;
    }

    // Substack tutorials typically build up to the main recipe, so the last
    // block is the primary one. Earlier blocks are sub-recipes (e.g. a custom
    // syrup or sugar mix that the main recipe uses).
    let title = blocks.last()?.title.clone();
    let multi_block = blocks.len() > 1;

    // For multi-block recipes every block needs a `<title>:` header line so
    // downstream section-aware ingredient grouping attributes each line to
    // the right sub-recipe. Skipping the last header would let its ingredients
    // bleed into the prior section.
    let mut ingredient_lines: Vec<String> = Vec::new();
    let mut instruction_paragraphs: Vec<String> = Vec::new();
    for block in blocks {
        if multi_block {
            ingredient_lines.push(format!("{}:", block.title));
            instruction_paragraphs.push(format!("{}:", block.title));
        }
        ingredient_lines.extend(block.ingredients);
        instruction_paragraphs.extend(block.instructions);
    }

    Some(RawRecipe {
        title,
        description: extract_og_meta(document, &OG_DESCRIPTION_SELECTOR),
        ingredients: ingredient_lines.join("\n"),
        instructions: instruction_paragraphs.join("\n\n"),
        image_urls: extract_og_image(document)
            .map(|u| vec![u])
            .unwrap_or_default(),
        source_url: Some(source_url.to_string()),
        source_name: extract_source_name(source_url),
        servings: None,
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

pub(super) fn scan_substack_blocks(body: ElementRef<'_>) -> Vec<SubstackBlock> {
    let mut blocks: Vec<SubstackBlock> = Vec::new();
    let mut current: Option<SubstackBlockBuilder> = None;

    for child in body.children() {
        let Some(elem) = ElementRef::wrap(child) else {
            continue;
        };
        let name = elem.value().name();

        if matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
            if let Some(block) = current.take().and_then(SubstackBlockBuilder::finish) {
                blocks.push(block);
            }
            let title = substack_element_text(elem);
            if !title.is_empty() {
                current = Some(SubstackBlockBuilder::new(title));
            }
            continue;
        }

        let Some(builder) = current.as_mut() else {
            continue;
        };
        if matches!(builder.state, BlockState::Closed) {
            continue;
        }

        match name {
            "ul" => {
                if matches!(builder.state, BlockState::Ingredients { .. }) {
                    for item in substack_list_items(elem) {
                        builder.ingredients.push(item);
                    }
                    builder.state = BlockState::Ingredients { locked: true };
                }
            }
            "ol" => {
                if matches!(builder.state, BlockState::Instructions) {
                    for item in substack_list_items(elem) {
                        builder.instructions.push(item);
                    }
                    builder.state = BlockState::Closed;
                }
            }
            "p" => {
                let text = substack_element_text(elem);
                if text.is_empty() || is_substack_subscribe_widget(&text) {
                    continue;
                }
                if matches!(builder.state, BlockState::Ingredients { .. })
                    && is_instructions_marker(&text)
                {
                    builder.state = BlockState::Instructions;
                    continue;
                }
                match builder.state {
                    // Older or simpler Substack posts use plain `<p>` for ingredients
                    // instead of a `<ul>`. Once a `<ul>` has been seen, subsequent
                    // `<p>` paragraphs are prose, not ingredients.
                    BlockState::Ingredients { locked: false } => builder.ingredients.push(text),
                    BlockState::Instructions => builder.instructions.push(text),
                    BlockState::Ingredients { locked: true } | BlockState::Closed => {}
                }
            }
            _ => {}
        }
    }

    if let Some(block) = current.and_then(SubstackBlockBuilder::finish) {
        blocks.push(block);
    }
    blocks
}

enum BlockState {
    Ingredients { locked: bool },
    Instructions,
    Closed,
}

struct SubstackBlockBuilder {
    title: String,
    ingredients: Vec<String>,
    instructions: Vec<String>,
    state: BlockState,
}

impl SubstackBlockBuilder {
    fn new(title: String) -> Self {
        Self {
            title,
            ingredients: Vec::new(),
            instructions: Vec::new(),
            state: BlockState::Ingredients { locked: false },
        }
    }

    fn finish(self) -> Option<SubstackBlock> {
        let saw_marker = !matches!(self.state, BlockState::Ingredients { .. });
        if !saw_marker || self.ingredients.is_empty() || self.instructions.is_empty() {
            return None;
        }
        Some(SubstackBlock {
            title: self.title,
            ingredients: self.ingredients,
            instructions: self.instructions,
        })
    }
}

pub(super) fn substack_list_items(list_elem: ElementRef<'_>) -> Vec<String> {
    list_elem
        .select(&LI_SELECTOR)
        .filter_map(|li| {
            let text = substack_element_text(li);
            if text.is_empty() || is_substack_subscribe_widget(&text) {
                None
            } else {
                Some(text)
            }
        })
        .collect()
}

pub(super) fn substack_element_text(elem: ElementRef<'_>) -> String {
    let mut buf = String::new();
    collect_substack_text(elem, &mut buf);
    decode_html_entities(buf.trim())
}

/// Walk descendants and append text, skipping subtrees that look like Substack
/// subscribe widgets (so a `<li>` whose recipe step is followed by an inline
/// subscribe form doesn't get "Subscribe" glued onto the end of its text).
pub(super) fn collect_substack_text(elem: ElementRef<'_>, buf: &mut String) {
    for child in elem.children() {
        if let Some(child_elem) = ElementRef::wrap(child) {
            if is_substack_widget_element(child_elem) {
                continue;
            }
            collect_substack_text(child_elem, buf);
        } else if let Some(text) = child.value().as_text() {
            buf.push_str(text);
        }
    }
}

pub(super) fn is_substack_widget_element(elem: ElementRef<'_>) -> bool {
    elem.value()
        .attr("class")
        .is_some_and(|c| c.contains("subscribe-widget"))
}

pub(super) fn is_instructions_marker(text: &str) -> bool {
    let trimmed = text.trim().trim_end_matches(':').trim();
    trimmed.eq_ignore_ascii_case("instructions") || trimmed.eq_ignore_ascii_case("directions")
}

pub(super) fn is_substack_subscribe_widget(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower == "subscribe" || lower == "subscribe now"
}

#[cfg(test)]
#[allow(clippy::print_stdout, clippy::print_stderr)]
mod tests {
    use super::*;

    #[test]
    fn test_substack_single_block_recipe() {
        // Substack post with one recipe block: heading + ingredient <p>s + INSTRUCTIONS marker + prose <p>s.
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <meta property="og:image" content="https://example.com/sidecar.jpg" />
                <meta property="og:description" content="A holiday cocktail." />
            </head>
            <body>
                <article>
                    <div class="body markup">
                        <p>Some prose introducing the post.</p>
                        <h4 class="header-anchor-post"><strong>Test Cocktail</strong></h4>
                        <p>1 dash bitters</p>
                        <p>1 ounce gin</p>
                        <p>½ ounce lemon juice</p>
                        <p><strong>INSTRUCTIONS</strong></p>
                        <p>Combine ingredients in a shaker.</p>
                        <p>Shake with ice.</p>
                        <p>Strain into a glass.</p>
                    </div>
                </article>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.substack.com/p/test-cocktail").unwrap();
        assert_eq!(result.title, "Test Cocktail");
        assert_eq!(
            result.ingredients,
            "1 dash bitters\n1 ounce gin\n½ ounce lemon juice"
        );
        assert_eq!(
            result.instructions,
            "Combine ingredients in a shaker.\n\nShake with ice.\n\nStrain into a glass."
        );
        assert_eq!(
            result.image_urls,
            vec!["https://example.com/sidecar.jpg".to_string()]
        );
        assert_eq!(result.description.as_deref(), Some("A holiday cocktail."));
    }

    #[test]
    fn test_substack_multi_block_recipe_uses_last_title() {
        // Two recipe blocks: a sub-recipe and the main recipe. Title should be the last block's.
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <div class="body markup">
                    <h4><strong>Spice Mix</strong></h4>
                    <p>1 teaspoon cinnamon</p>
                    <p>5 teaspoons sugar</p>
                    <p><strong>INSTRUCTIONS</strong></p>
                    <p>Combine in a bowl.</p>
                    <h4><strong>Main Drink</strong></h4>
                    <p>2 ounces rye</p>
                    <p>1 ounce lemon juice</p>
                    <p><strong>INSTRUCTIONS</strong></p>
                    <p>Shake with ice.</p>
                    <p>Strain.</p>
                </div>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.substack.com/p/main-drink").unwrap();
        assert_eq!(result.title, "Main Drink");
        assert_eq!(
            result.ingredients,
            "Spice Mix:\n1 teaspoon cinnamon\n5 teaspoons sugar\n\
             Main Drink:\n2 ounces rye\n1 ounce lemon juice"
        );
        assert_eq!(
            result.instructions,
            "Spice Mix:\n\nCombine in a bowl.\n\n\
             Main Drink:\n\nShake with ice.\n\nStrain."
        );
    }

    #[test]
    fn test_substack_paywalled_post_returns_no_recipe() {
        // Body markup is present but the post stops at the teaser before any
        // INSTRUCTIONS marker — Substack's paywall preview shape.
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <div class="body markup">
                    <h2><strong>Setup</strong></h2>
                    <p>Some intro prose, no ingredient list.</p>
                    <p>Continue reading this post for free, courtesy of Peter Suderman.</p>
                </div>
            </body>
            </html>
        "#;

        let err = extract_recipe(html, "https://example.substack.com/p/paywalled")
            .expect_err("paywalled post should not extract");
        // Should fail with a missing-field error from the fallback chain.
        let msg = err.to_string();
        assert!(
            msg.contains("name")
                || msg.contains("recipeIngredient")
                || msg.contains("recipeInstructions"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn test_substack_skips_subscribe_widget_paragraphs() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <div class="body markup">
                    <h4><strong>Test Drink</strong></h4>
                    <p>1 dash bitters</p>
                    <p>Subscribe</p>
                    <p>2 ounces rye</p>
                    <p><strong>INSTRUCTIONS</strong></p>
                    <p>Subscribe now</p>
                    <p>Combine and shake.</p>
                </div>
            </body>
            </html>
        "#;
        let result = extract_recipe(html, "https://example.substack.com/p/test").unwrap();
        assert_eq!(result.ingredients, "1 dash bitters\n2 ounces rye");
        assert_eq!(result.instructions, "Combine and shake.");
    }
}
