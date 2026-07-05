use super::*;

static STRUCTURED_INGREDIENTS_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(".structured-ingredients").expect("structured ingredients selector")
});

static STRUCTURED_HEADING_ITEM_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(".structured-ingredients__list-heading, .structured-ingredients__list-item")
        .expect("structured ingredients heading/item selector")
});

/// Extract ingredients from Dotdash Meredith CMS pages (Serious Eats, Simply Recipes,
/// Allrecipes). Print pages frequently ship without JSON-LD; the recipe is rendered
/// only via `.structured-ingredients__list-item` lists. Group headers appear as
/// `.structured-ingredients__list-heading` paragraphs interleaved with the lists.
pub(in crate::extract) fn extract_dotdash_meredith_ingredients(document: &Html) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for container in document.select(&STRUCTURED_INGREDIENTS_SELECTOR) {
        for el in container.select(&STRUCTURED_HEADING_ITEM_SELECTOR) {
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

static INGREDIENT_UNIT_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("[data-ingredient-unit]").expect("data-ingredient-unit selector")
});

static INGREDIENT_NAME_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("[data-ingredient-name]").expect("data-ingredient-name selector")
});

/// Get the visible text of a Dotdash Meredith ingredient list-item, working
/// around a CMS quirk: when an ingredient like "1 baguette" has no real unit,
/// the page renders both `data-ingredient-unit` and `data-ingredient-name`
/// spans with identical text ("1 baguette baguette"). When that pattern shows
/// up, drop the duplicated word once.
pub(in crate::extract) fn dotdash_ingredient_item_text(li: &ElementRef<'_>) -> String {
    let raw_text = li.text().collect::<String>();
    let mut result = raw_text.split_whitespace().collect::<Vec<_>>().join(" ");

    let unit_texts: Vec<String> = li
        .select(&INGREDIENT_UNIT_SELECTOR)
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
        .select(&INGREDIENT_NAME_SELECTOR)
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
static DOTDASH_STEPS_LI_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(".structured-project__steps li.mntl-sc-block-group--LI")
        .expect("dotdash steps selector")
});

static DOTDASH_STEP_P_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("p.mntl-sc-block-html").expect("dotdash step paragraph selector")
});

/// Extract instructions from Dotdash Meredith CMS pages (Serious Eats, Simply Recipes,
/// Allrecipes). The directions section uses `.section--instructions` containing
/// `.structured-project__steps` with each step as `<li class="mntl-sc-block-group--LI">`
/// holding one or more `<p class="mntl-sc-block-html">` paragraphs.
pub(in crate::extract) fn extract_dotdash_meredith_instructions(document: &Html) -> Option<String> {
    let mut steps: Vec<String> = Vec::new();
    for li in document.select(&DOTDASH_STEPS_LI_SELECTOR) {
        let parts: Vec<String> = li
            .select(&DOTDASH_STEP_P_SELECTOR)
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
