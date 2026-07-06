//! WP Recipe Maker (WPRM) ingredient-group and instruction extraction.

use super::*;

static INGREDIENT_GROUP_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(".wprm-recipe-ingredient-group").expect("wprm ingredient group selector")
});

static GROUP_NAME_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".wprm-recipe-group-name").expect("wprm group name selector"));

pub(super) static WPRM_INGREDIENT_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".wprm-recipe-ingredient").expect("wprm ingredient selector"));

/// Extract WPRM ingredients with group headers (e.g. "Meatballs:", "Broth:").
///
/// WPRM structures ingredients as `.wprm-recipe-ingredient-group` containers,
/// each with an optional `.wprm-recipe-group-name` header and a list of
/// `.wprm-recipe-ingredient` items. JSON-LD flattens these into a single array,
/// losing the group structure. This function recovers it from the HTML.
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
        for item in group.select(&WPRM_INGREDIENT_SELECTOR) {
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
    // Instruction steps are single-line: collapse newlines too.
    let text = normalize_wprm_text(&fragment_to_text(&html));
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
