//! WP Recipe Maker (WPRM) ingredient-group and instruction extraction.

use super::*;

/// Extract WPRM ingredients with group headers (e.g. "Meatballs:", "Broth:").
///
/// WPRM structures ingredients as `.wprm-recipe-ingredient-group` containers,
/// each with an optional `.wprm-recipe-group-name` header and a list of
/// `.wprm-recipe-ingredient` items. JSON-LD flattens these into a single array,
/// losing the group structure. This function recovers it from the HTML.
static INGREDIENT_GROUP_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(".wprm-recipe-ingredient-group").expect("wprm ingredient group selector")
});

static GROUP_NAME_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".wprm-recipe-group-name").expect("wprm group name selector"));

static INGREDIENT_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".wprm-recipe-ingredient").expect("wprm ingredient selector"));

pub(super) fn extract_wprm_ingredients_with_groups(document: &Html) -> Option<String> {
    let groups: Vec<_> = document.select(&INGREDIENT_GROUP_SELECTOR).collect();
    if groups.is_empty() {
        return None;
    }

    // Only use this path when at least one group actually has a name
    let has_any_group_name = groups.iter().any(|g| {
        g.select(&GROUP_NAME_SELECTOR)
            .next()
            .map(|el| !el.text().collect::<String>().trim().is_empty())
            .unwrap_or(false)
    });
    if !has_any_group_name {
        return None;
    }

    let mut lines: Vec<String> = Vec::new();
    for group in &groups {
        // Extract group name if present
        if let Some(name_el) = group.select(&GROUP_NAME_SELECTOR).next() {
            let name = name_el.text().collect::<String>().trim().to_string();
            if !name.is_empty() {
                // Add as section header (colon-terminated so the parser detects it)
                if name.ends_with(':') {
                    lines.push(name);
                } else {
                    lines.push(format!("{}:", name));
                }
            }
        }

        // Extract ingredients in this group
        for item in group.select(&INGREDIENT_SELECTOR) {
            if let Some(text) = sanitize_extracted_ingredient(&item.text().collect::<String>()) {
                lines.push(text);
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

pub(super) static WPRM_STICKY_NOTE_TEXT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<span[^>]*class="[^"]*\bsticky-note-text\b[^"]*"[^>]*>.*?</span>"#)
        .expect("Invalid WPRM sticky note text regex")
});

pub(super) static WPRM_STICKY_NOTE_WRAPPER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<span[^>]*class="[^"]*\bsticky-note\b[^"]*"[^>]*>\s*</span>"#)
        .expect("Invalid WPRM sticky note wrapper regex")
});

/// Block-level boundaries inside a WPRM instruction. WPRM renders each visual line
/// as a `display: block` span and separates them with `wprm-spacer` divs or `<br>`.
/// Stripping these tags to nothing glues adjacent blocks together (e.g. a step and
/// its inline "Nami's Tip:" run on without a space), so we turn the boundaries into
/// spaces before stripping the remaining inline tags.
pub(super) static WPRM_BLOCK_BOUNDARY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<div[^>]*\bwprm-spacer\b[^>]*>\s*</div>|<br\s*/?>|<span[^>]*style="[^"]*display:\s*block[^"]*"[^>]*>"#,
    )
    .expect("Invalid WPRM block boundary regex")
});

pub(super) fn normalize_wprm_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Convert the inner HTML of a `.wprm-recipe-instruction-text` element to clean text,
/// preserving spaces at block boundaries so inline tips don't glue onto the step text.
pub(super) fn clean_wprm_instruction_text(inner_html: &str) -> Option<String> {
    let html = WPRM_STICKY_NOTE_TEXT_REGEX.replace_all(inner_html, "");
    let html = WPRM_STICKY_NOTE_WRAPPER_REGEX.replace_all(&html, "");
    let html = WPRM_BLOCK_BOUNDARY_REGEX.replace_all(&html, " ");
    let text = HTML_TAG_REGEX.replace_all(&html, "");
    let text = decode_html_entities(text.trim());
    let text = normalize_wprm_text(&text);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

pub(super) fn lowercase_wprm_text(text: &str) -> String {
    normalize_wprm_text(text)
        .chars()
        .flat_map(char::to_lowercase)
        .collect()
}

pub(super) fn canonicalize_wprm_title(text: &str) -> String {
    let normalized = lowercase_wprm_text(text);

    normalized
        .split(['|', '•'])
        .next()
        .unwrap_or(&normalized)
        .split(" - ")
        .next()
        .unwrap_or(&normalized)
        .split(" – ")
        .next()
        .unwrap_or(&normalized)
        .split(" — ")
        .next()
        .unwrap_or(&normalized)
        .trim()
        .to_string()
}

pub(super) fn normalize_wprm_title_tokens(text: &str) -> Vec<String> {
    canonicalize_wprm_title(text)
        .split(|c: char| !c.is_alphanumeric())
        .filter_map(|part| {
            let part = part
                .trim()
                .chars()
                .flat_map(char::to_lowercase)
                .collect::<String>();
            if part.is_empty() || matches!(part.as_str(), "recipe" | "recipes") {
                None
            } else {
                Some(part)
            }
        })
        .collect()
}

pub(super) fn wprm_titles_match(recipe_title: &str, card_title: &str) -> bool {
    let normalized_recipe = canonicalize_wprm_title(recipe_title);
    let normalized_card = canonicalize_wprm_title(card_title);

    if !normalized_recipe.is_empty() && normalized_recipe == normalized_card {
        return true;
    }

    let recipe_tokens = normalize_wprm_title_tokens(recipe_title);
    let card_tokens = normalize_wprm_title_tokens(card_title);

    if recipe_tokens.is_empty() || card_tokens.is_empty() {
        return false;
    }

    recipe_tokens == card_tokens
}

static INSTRUCTION_GROUP_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(".wprm-recipe-instruction-group").expect("wprm instruction group selector")
});

static INSTRUCTION_GROUP_NAME_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(".wprm-recipe-instruction-group-name")
        .expect("wprm instruction group name selector")
});

static INSTRUCTION_TEXT_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(".wprm-recipe-instruction-text").expect("wprm instruction text selector")
});

pub(super) fn extract_wprm_steps(root: ElementRef<'_>) -> Option<Vec<String>> {
    let mut lines: Vec<String> = Vec::new();

    let groups: Vec<_> = root.select(&INSTRUCTION_GROUP_SELECTOR).collect();
    if groups.is_empty() {
        // No group wrappers; emit the steps directly under the card.
        for el in root.select(&INSTRUCTION_TEXT_SELECTOR) {
            if let Some(text) = clean_wprm_instruction_text(&el.inner_html()) {
                lines.push(text);
            }
        }
    } else {
        for group in &groups {
            // Emit the group name as a colon-terminated section header (matching the
            // ingredient-group convention) so downstream parsing treats it as a header.
            if let Some(name_el) = group.select(&INSTRUCTION_GROUP_NAME_SELECTOR).next() {
                let name = normalize_wprm_text(&name_el.text().collect::<String>());
                if !name.is_empty() {
                    if name.ends_with(':') {
                        lines.push(name);
                    } else {
                        lines.push(format!("{name}:"));
                    }
                }
            }
            for el in group.select(&INSTRUCTION_TEXT_SELECTOR) {
                if let Some(text) = clean_wprm_instruction_text(&el.inner_html()) {
                    lines.push(text);
                }
            }
        }
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines)
    }
}

static RECIPE_CARD_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".wprm-recipe").expect("wprm recipe selector"));

static RECIPE_NAME_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".wprm-recipe-name").expect("wprm recipe name selector"));

pub(super) fn extract_wprm_instructions(document: &Html, recipe_title: &str) -> Option<String> {
    let normalized_title = normalize_wprm_text(recipe_title);
    let mut matching_steps = None;
    let mut only_card_steps = None;
    let mut card_count = 0;

    for card in document.select(&RECIPE_CARD_SELECTOR) {
        let steps = match extract_wprm_steps(card) {
            Some(steps) => steps,
            None => continue,
        };

        card_count += 1;
        if only_card_steps.is_none() {
            only_card_steps = Some(steps.clone());
        }

        let card_title = card
            .select(&RECIPE_NAME_SELECTOR)
            .next()
            .map(|el| normalize_wprm_text(&el.text().collect::<String>()));

        if let Some(card_title) = card_title {
            if !normalized_title.is_empty() && wprm_titles_match(&normalized_title, &card_title) {
                matching_steps = Some(steps);
                break;
            }
        }
    }

    let require_title_match = !normalized_title.is_empty();

    let steps = if let Some(steps) = matching_steps {
        steps
    } else if !require_title_match && card_count == 1 {
        only_card_steps?
    } else if !require_title_match {
        let orphan_steps: Vec<String> = document
            .select(&INSTRUCTION_TEXT_SELECTOR)
            .filter_map(|el| clean_wprm_instruction_text(&el.inner_html()))
            .collect();

        if orphan_steps.len() == 1 {
            orphan_steps
        } else {
            return None;
        }
    } else {
        return None;
    };

    Some(steps.join("\n\n"))
}

#[cfg(test)]
#[allow(clippy::print_stdout, clippy::print_stderr)]
mod tests {
    use super::*;

    #[test]
    fn test_wprm_ingredient_groups_supplement_jsonld() {
        // WPRM JSON-LD provides flat ingredient list; HTML has the group structure.
        // The extraction should inject group headers from the HTML.
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Ginger Meatballs",
                    "recipeIngredient": [
                        "2 pounds ground pork",
                        "2 large eggs",
                        "1 can coconut milk",
                        "2 cups chicken stock"
                    ],
                    "recipeInstructions": "Make meatballs and broth."
                }
                </script>
            </head>
            <body>
                <div class="wprm-recipe-ingredient-group">
                    <span class="wprm-recipe-group-name">Meatballs</span>
                    <ul>
                        <li class="wprm-recipe-ingredient">2 pounds ground pork</li>
                        <li class="wprm-recipe-ingredient">2 large eggs</li>
                    </ul>
                </div>
                <div class="wprm-recipe-ingredient-group">
                    <span class="wprm-recipe-group-name">Broth</span>
                    <ul>
                        <li class="wprm-recipe-ingredient">1 can coconut milk</li>
                        <li class="wprm-recipe-ingredient">2 cups chicken stock</li>
                    </ul>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        let lines: Vec<&str> = result.ingredients.lines().collect();
        assert_eq!(lines[0], "Meatballs:");
        assert_eq!(lines[1], "2 pounds ground pork");
        assert_eq!(lines[2], "2 large eggs");
        assert_eq!(lines[3], "Broth:");
        assert_eq!(lines[4], "1 can coconut milk");
        assert_eq!(lines[5], "2 cups chicken stock");
    }

    #[test]
    fn test_wprm_ingredient_groups_no_names_skipped() {
        // When WPRM groups exist but none have names, fall through to flat extraction
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Simple Recipe",
                    "recipeIngredient": ["1 cup flour", "2 eggs"],
                    "recipeInstructions": "Mix."
                }
                </script>
            </head>
            <body>
                <div class="wprm-recipe-ingredient-group">
                    <span class="wprm-recipe-group-name"></span>
                    <ul>
                        <li class="wprm-recipe-ingredient">1 cup flour</li>
                        <li class="wprm-recipe-ingredient">2 eggs</li>
                    </ul>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        // Should use JSON-LD ingredients (no group headers injected)
        assert!(!result.ingredients.contains(':'));
        assert!(result.ingredients.contains("1 cup flour"));
    }

    #[test]
    fn test_wprm_sticky_note_instruction_prefers_clean_html_text() {
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Semi-Instant Pancakes",
                    "recipeIngredient": ["1 cup flour"],
                    "recipeInstructions": [
                        {
                            "@type": "HowToStep",
                            "text": "Carefully flipThe first cakes tend to be uglier than later ones, so feed the in-laws first. with a wide spatula and cook until golden brown on the bottom, another 2 to 3 minutes."
                        }
                    ]
                }
                </script>
            </head>
            <body>
                <div class="wprm-recipe">
                    <h2 class="wprm-recipe-name">Semi-Instant Pancakes</h2>
                    <li class="wprm-recipe-instruction">
                        <div class="wprm-recipe-instruction-text">
                            <span style="display: block;">
                                <wprm-code>
                                    <span class="sticky-note-btn">Carefully flip</span>
                                    <span class="sticky-note">
                                        <span class="sticky-note-text">The first cakes tend to be uglier than later ones, so feed the in-laws first.</span>
                                    </span>
                                    with a wide spatula and cook until golden brown on the bottom, another 2 to 3 minutes.
                                </wprm-code>
                            </span>
                        </div>
                    </li>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/pancakes").unwrap();
        assert_eq!(
            result.instructions,
            "Carefully flip with a wide spatula and cook until golden brown on the bottom, another 2 to 3 minutes."
        );
    }

    #[test]
    fn test_wprm_instruction_supplement_scopes_to_matching_recipe_card() {
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Semi-Instant Pancakes",
                    "recipeIngredient": ["1 cup flour"],
                    "recipeInstructions": [
                        {
                            "@type": "HowToStep",
                            "text": "Polluted JSON-LD step"
                        }
                    ]
                }
                </script>
            </head>
            <body>
                <div class="wprm-recipe">
                    <h2 class="wprm-recipe-name">Other Recipe</h2>
                    <li class="wprm-recipe-instruction">
                        <div class="wprm-recipe-instruction-text">Wrong card step</div>
                    </li>
                </div>
                <div class="wprm-recipe">
                    <h2 class="wprm-recipe-name">Semi-Instant Pancakes</h2>
                    <li class="wprm-recipe-instruction">
                        <div class="wprm-recipe-instruction-text">
                            <wprm-code>
                                <span class="sticky-note-btn">Carefully flip</span>
                                <span class="sticky-note">
                                    <span class="sticky-note-text">Ignore this note.</span>
                                </span>
                                with a wide spatula
                            </wprm-code>
                        </div>
                    </li>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/pancakes").unwrap();
        assert_eq!(result.instructions, "Carefully flip with a wide spatula");
    }

    #[test]
    fn test_wprm_instruction_supplement_matches_common_title_variants() {
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Semi-Instant Pancakes",
                    "recipeIngredient": ["1 cup flour"],
                    "recipeInstructions": [
                        {
                            "@type": "HowToStep",
                            "text": "Polluted JSON-LD step"
                        }
                    ]
                }
                </script>
            </head>
            <body>
                <div class="wprm-recipe">
                    <h2 class="wprm-recipe-name">Semi Instant Pancakes Recipe</h2>
                    <li class="wprm-recipe-instruction">
                        <div class="wprm-recipe-instruction-text">
                            <wprm-code>
                                <span class="sticky-note-btn">Carefully flip</span>
                                <span class="sticky-note">
                                    <span class="sticky-note-text">Ignore this note.</span>
                                </span>
                                with a wide spatula
                            </wprm-code>
                        </div>
                    </li>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/pancakes").unwrap();
        assert_eq!(result.instructions, "Carefully flip with a wide spatula");
    }

    #[test]
    fn test_wprm_instruction_supplement_ignores_partial_title_matches() {
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Chicken Soup",
                    "recipeIngredient": ["1 cup stock"],
                    "recipeInstructions": [
                        {
                            "@type": "HowToStep",
                            "text": "Keep the original structured instruction."
                        }
                    ]
                }
                </script>
            </head>
            <body>
                <div class="wprm-recipe">
                    <h2 class="wprm-recipe-name">Chicken</h2>
                    <li class="wprm-recipe-instruction">
                        <div class="wprm-recipe-instruction-text">Wrong card step</div>
                    </li>
                </div>
                <div class="wprm-recipe">
                    <h2 class="wprm-recipe-name">Chicken Soup Recipe</h2>
                    <li class="wprm-recipe-instruction">
                        <div class="wprm-recipe-instruction-text">Correct card step</div>
                    </li>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/chicken-soup").unwrap();
        assert_eq!(result.instructions, "Correct card step");
    }

    #[test]
    fn test_wprm_instruction_supplement_matches_unicode_titles() {
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Crème Brûlée",
                    "recipeIngredient": ["2 egg yolks"],
                    "recipeInstructions": [
                        {
                            "@type": "HowToStep",
                            "text": "Polluted JSON-LD step"
                        }
                    ]
                }
                </script>
            </head>
            <body>
                <div class="wprm-recipe">
                    <h2 class="wprm-recipe-name">Crème Brûlée Recipe</h2>
                    <li class="wprm-recipe-instruction">
                        <div class="wprm-recipe-instruction-text">Torch the sugar topping.</div>
                    </li>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/creme-brulee").unwrap();
        assert_eq!(result.instructions, "Torch the sugar topping.");
    }

    #[test]
    fn test_wprm_instruction_supplement_matches_title_with_site_qualifier() {
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Semi-Instant Pancakes | Alton Brown",
                    "recipeIngredient": ["1 cup flour"],
                    "recipeInstructions": [
                        {
                            "@type": "HowToStep",
                            "text": "Polluted JSON-LD step"
                        }
                    ]
                }
                </script>
            </head>
            <body>
                <div class="wprm-recipe">
                    <h2 class="wprm-recipe-name">Semi-Instant Pancakes</h2>
                    <li class="wprm-recipe-instruction">
                        <div class="wprm-recipe-instruction-text">Correct card step</div>
                    </li>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/pancakes").unwrap();
        assert_eq!(result.instructions, "Correct card step");
    }

    #[test]
    fn test_wprm_instruction_supplement_does_not_use_unmatched_single_card() {
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Semi-Instant Pancakes",
                    "recipeIngredient": ["1 cup flour"],
                    "recipeInstructions": [
                        {
                            "@type": "HowToStep",
                            "text": "Keep the original structured instruction."
                        }
                    ]
                }
                </script>
            </head>
            <body>
                <div class="wprm-recipe">
                    <h2 class="wprm-recipe-name">Different Recipe</h2>
                    <li class="wprm-recipe-instruction">
                        <div class="wprm-recipe-instruction-text">Wrong card step</div>
                    </li>
                </div>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/pancakes").unwrap();
        assert_eq!(
            result.instructions,
            "Keep the original structured instruction."
        );
    }

    #[test]
    fn test_wprm_instruction_groups_add_headers_and_inline_tips() {
        let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Simmered Potatoes",
                    "recipeIngredient": ["14 oz potatoes"],
                    "recipeInstructions": [
                        {"@type": "HowToStep", "text": "Place a drop lid and simmer.Nami&#39;s Tip: It keeps the potatoes from breaking."}
                    ]
                }
                </script>
            </head>
            <body>
                <div class="wprm-recipe">
                    <h2 class="wprm-recipe-name">Simmered Potatoes</h2>
                    <div class="wprm-recipe-instruction-group">
                        <li class="wprm-recipe-instruction">
                            <div class="wprm-recipe-instruction-text"><span style="display: block;">Gather all the ingredients.</span></div>
                        </li>
                    </div>
                    <div class="wprm-recipe-instruction-group">
                        <h4 class="wprm-recipe-instruction-group-name">To Cook the Potatoes</h4>
                        <li class="wprm-recipe-instruction">
                            <div class="wprm-recipe-instruction-text"><span style="display: block;">Place a drop lid and simmer.</span><div class="wprm-spacer"></div><span style="display: block;"><strong>Nami's Tip:</strong> It keeps the potatoes from breaking.</span></div>
                        </li>
                    </div>
                </div>
            </body></html>
        "#;

        let recipe = extract_recipe(html, "https://example.com/simmered").unwrap();
        assert_eq!(
            recipe.instructions,
            "Gather all the ingredients.\n\nTo Cook the Potatoes:\n\nPlace a drop lid and simmer. Nami's Tip: It keeps the potatoes from breaking."
        );
    }
}
