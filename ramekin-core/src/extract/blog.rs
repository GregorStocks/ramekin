//! Unstructured blog-post extraction heuristics (bold/underline headings, chunk scanning).

use super::*;

/// Regex to strip trailing parenthetical or bracketed qualifiers from a title
/// before reusing it as an ingredient section marker.
pub(super) static TRAILING_TITLE_QUALIFIER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s*[\(\[][^\)\]]*[\)\]]\s*$").expect("Invalid trailing title qualifier regex")
});

/// Regex to split HTML on paragraph boundaries for unstructured blog extraction.
pub(super) static P_TAG_SPLIT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)</?p[^>]*>").expect("Invalid p-tag split regex"));

/// Regex to match `<br>` tags in various forms.
pub(super) static BR_TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").expect("Invalid br regex"));

/// Regex to extract bold/strong text (recipe title signal).
pub(super) static BOLD_TEXT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<(?:b|strong)>([^<]+)</(?:b|strong)>").expect("Invalid bold text regex")
});

/// Regex to extract underlined text (section header signal).
pub(super) static UNDERLINE_TEXT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<u>([^<]+)</u>").expect("Invalid underline text regex"));

/// Regex to detect "One year ago:" / "Previously" / "Two years ago:" link sections.
pub(super) static LOOKBACK_LINK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(\s*<b>)?\s*(one|two|three|four|five|six|seven|eight|nine|ten|\d+)\s+years?\s+ago\s*:|(^|\s)previously\b")
        .expect("Invalid lookback link regex")
});

/// Regex to detect a single "lookback link" line such as "Six Months Ago:",
/// "1.5 Years Ago:", or "Two Years Ago:". Used to identify chunks that are
/// mostly cross-links to past posts rather than ingredient lists. Applied
/// to plain text after stripping HTML tags.
pub(super) static LOOKBACK_LINK_LINE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(one|two|three|four|five|six|seven|eight|nine|ten|\d+(?:\.\d+)?)\s+(years?|months?)\s+ago\s*:")
        .expect("Invalid lookback link line regex")
});

/// Detect chunks that are dominated by "X Years/Months Ago:" cross-links to
/// past posts. These can otherwise sneak past `looks_like_ingredient_list`
/// when the digits in "1.5 Years Ago" satisfy the leading-digit quantity
/// pattern.
pub(super) fn looks_like_lookback_links_chunk(chunk: &str) -> bool {
    let lines: Vec<&str> = BR_TAG_REGEX.split(chunk).collect();
    let mut lookback_lines = 0;
    let mut text_lines = 0;
    for line in &lines {
        let text = HTML_TAG_REGEX.replace_all(line, "");
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        text_lines += 1;
        if LOOKBACK_LINK_LINE_REGEX.is_match(text) {
            lookback_lines += 1;
        }
    }
    lookback_lines >= 2 && lookback_lines * 100 / text_lines.max(1) >= 40
}

struct UnstructuredRecipeBlock {
    title: Option<String>,
    title_chunk_idx: Option<usize>,
    ingredient_chunk_indices: Vec<usize>,
}

pub(super) fn extract_bold_heading(chunk: &str) -> Option<String> {
    let cap = BOLD_TEXT_REGEX.captures(chunk)?;
    let bold_text = cap.get(1)?.as_str().trim();
    if LOOKBACK_LINK_REGEX.is_match(chunk) || bold_text.is_empty() {
        return None;
    }
    Some(decode_html_entities(bold_text))
}

pub(super) fn has_instruction_paragraph_between(
    chunks: &[&str],
    start_idx: usize,
    end_idx: usize,
) -> bool {
    for chunk in chunks.iter().take(end_idx).skip(start_idx) {
        let chunk = chunk.trim();
        if chunk.is_empty()
            || LOOKBACK_LINK_REGEX.is_match(chunk)
            || looks_like_lookback_links_chunk(chunk)
            || looks_like_ingredient_list(chunk)
        {
            continue;
        }

        let text = HTML_TAG_REGEX.replace_all(chunk, "");
        let text = text.trim();
        if text.is_empty() {
            continue;
        }

        let lower = text.to_lowercase();
        if lower == "video" || lower == "watch the video" {
            continue;
        }
        if lower.starts_with("adapted from")
            || lower.starts_with("from ")
            || lower.starts_with("recipe from")
            || lower.starts_with("source:")
        {
            continue;
        }

        return true;
    }

    false
}

pub(super) fn normalized_block_section_title(title: &str) -> Option<String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut candidates = vec![trimmed.trim_end_matches(':').trim().to_string()];
    let stripped = TRAILING_TITLE_QUALIFIER_RE
        .replace(trimmed, "")
        .trim_end_matches(':')
        .trim()
        .to_string();
    if !stripped.is_empty() && stripped != candidates[0] {
        candidates.push(stripped);
    }

    for candidate in candidates {
        let header = format!("{candidate}:");
        if let Some(section_name) = detect_section_header(&header) {
            return Some(section_name);
        }
    }

    None
}

pub(super) fn underlined_section_title(title: &str) -> Option<String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return None;
    }

    let stripped = TRAILING_TITLE_QUALIFIER_RE
        .replace(trimmed, "")
        .trim_end_matches(':')
        .trim()
        .to_string();
    let mut candidates = Vec::new();
    if !stripped.is_empty() {
        candidates.push(stripped);
    }
    let trimmed_without_colon = trimmed.trim_end_matches(':').trim();
    if trimmed_without_colon != candidates.first().map(String::as_str).unwrap_or_default() {
        candidates.push(trimmed_without_colon.to_string());
    }

    for candidate in candidates {
        let header = format!("{candidate}:");
        if detect_section_header(&header).is_some() {
            return Some(candidate);
        }
    }

    None
}

pub(super) fn extract_underlined_section_title(part: &str, header: &str) -> Option<String> {
    let text = HTML_TAG_REGEX.replace_all(part, "");
    let text = decode_html_entities(text.trim());
    let suffix = text.strip_prefix(header)?.trim();

    if !suffix.is_empty() && !TRAILING_TITLE_QUALIFIER_RE.is_match(suffix) {
        return None;
    }

    underlined_section_title(&text)
}

/// Extract a recipe from an unstructured blog post.
///
/// Handles older WordPress posts that write recipes in plain HTML without any
/// recipe plugin or structured data. The defining signal is `<br>`-delimited
/// ingredient lists in `<p>` blocks: this is how bloggers commonly format
/// ingredient lists in the post editor.
///
/// Pattern:
/// 1. `<p><b>Recipe Title</b>...` (bold text introduces the recipe)
/// 2. `<p>ingredient 1<br>ingredient 2<br>ingredient 3</p>` (ingredients)
/// 3. `<p>Prose instruction paragraph.</p>` (instructions, no `<br>` chains)
pub(super) fn extract_recipe_from_unstructured_blog(
    html: &str,
    source_url: &str,
) -> Option<RawRecipe> {
    // Limit search to before comments section to avoid picking up user comments.
    // These markers are ASCII so the byte position is always a valid char boundary.
    let comments_pos = html
        .find("<div id=\"comments\"")
        .or_else(|| html.find("<section id=\"comments\""))
        .or_else(|| html.find("<ol class=\"commentlist\""))
        .or_else(|| html.find("<div class=\"comments-area\""));
    let search_html = match comments_pos {
        Some(pos) => html.get(..pos).unwrap_or(html),
        None => html,
    };

    // Split on <p> tags to get paragraph chunks
    let chunks: Vec<&str> = P_TAG_SPLIT_REGEX.split(search_html).collect();

    // Find paragraph chunks that look like ingredient lists:
    // they contain 2+ <br> tags and their lines look like ingredients (short, with quantities)
    let ingredient_chunk_indices: Vec<usize> = chunks
        .iter()
        .enumerate()
        .filter(|(_, chunk)| {
            let trimmed = chunk.trim();
            !trimmed.is_empty()
                && BR_TAG_REGEX.find_iter(trimmed).count() >= 2
                && !looks_like_lookback_links_chunk(trimmed)
                && looks_like_ingredient_list(trimmed)
        })
        .map(|(i, _)| i)
        .collect();

    if ingredient_chunk_indices.is_empty() {
        return None;
    }

    let mut blocks: Vec<UnstructuredRecipeBlock> = Vec::new();
    let mut scan_start = 0;
    for &ingredient_idx in &ingredient_chunk_indices {
        let mut block_title = None;
        let mut block_title_chunk_idx = None;
        for i in (scan_start..ingredient_idx).rev() {
            let chunk = chunks[i].trim();
            if chunk.is_empty() {
                continue;
            }
            if let Some(title) = extract_bold_heading(chunk) {
                block_title = Some(title);
                block_title_chunk_idx = Some(i);
                break;
            }
        }

        let starts_new_block = match blocks.last() {
            None => true,
            Some(block) => {
                if let Some(title_chunk_idx) = block_title_chunk_idx {
                    let previous_ingredient_idx = *block.ingredient_chunk_indices.last()?;
                    has_instruction_paragraph_between(
                        &chunks,
                        previous_ingredient_idx + 1,
                        title_chunk_idx,
                    )
                } else {
                    false
                }
            }
        };

        if starts_new_block {
            blocks.push(UnstructuredRecipeBlock {
                title: block_title,
                title_chunk_idx: block_title_chunk_idx,
                ingredient_chunk_indices: vec![ingredient_idx],
            });
        } else if let Some(block) = blocks.last_mut() {
            block.ingredient_chunk_indices.push(ingredient_idx);
        }

        scan_start = ingredient_idx + 1;
    }

    let first_block = blocks.first()?;
    let first_ingredient_idx = *first_block.ingredient_chunk_indices.first()?;
    let is_multi_block = blocks.len() > 1;

    let mut ingredient_lines: Vec<String> = Vec::new();
    for (block_idx, block) in blocks.iter().enumerate() {
        if is_multi_block && block_idx > 0 {
            if let Some(title) = block
                .title
                .as_deref()
                .and_then(normalized_block_section_title)
            {
                ingredient_lines.push(format!("{title}:"));
            }
        }
        for &idx in &block.ingredient_chunk_indices {
            let chunk = chunks[idx];
            extract_ingredient_lines_from_chunk(chunk, &mut ingredient_lines);
        }
    }

    if ingredient_lines.is_empty() {
        return None;
    }

    let mut title = first_block.title.clone();

    // Fall back to page title if no bold title found near ingredients
    if title.is_none() {
        let document = Html::parse_document(html);
        title = extract_title_from_html(&document);
    }

    let title = title?;

    // Extract instructions: prose paragraphs after the last ingredient chunk
    // that don't contain <br> chains (i.e., they're not ingredient lists)
    let mut instruction_paragraphs: Vec<String> = Vec::new();
    for (block_idx, block) in blocks.iter().enumerate() {
        let last_ingredient_idx = *block.ingredient_chunk_indices.last()?;
        let next_block_title_idx = blocks
            .get(block_idx + 1)
            .and_then(|next_block| next_block.title_chunk_idx);

        let mut block_paragraphs: Vec<String> = Vec::new();
        for (idx, chunk) in chunks.iter().enumerate().skip(last_ingredient_idx + 1) {
            if next_block_title_idx.is_some_and(|title_idx| idx >= title_idx) {
                break;
            }

            let chunk = chunk.trim();
            if chunk.is_empty() {
                continue;
            }

            if chunk.contains("sharedaddy") || chunk.contains("sd-sharing") {
                break;
            }

            let text = HTML_TAG_REGEX.replace_all(chunk, "");
            let text = text.trim();
            if text.is_empty() {
                continue;
            }

            if LOOKBACK_LINK_REGEX.is_match(chunk) || looks_like_lookback_links_chunk(chunk) {
                continue;
            }

            if block_paragraphs.is_empty() {
                let lower = text.to_lowercase();
                if lower.starts_with("adapted from")
                    || lower.starts_with("from ")
                    || lower.starts_with("recipe from")
                    || lower.starts_with("source:")
                {
                    continue;
                }
            }

            let decoded = decode_html_entities(text);
            if !decoded.is_empty() {
                block_paragraphs.push(decoded);
            }
        }

        if block_paragraphs.is_empty() {
            continue;
        }

        if is_multi_block && block_idx > 0 {
            if let Some(title) = block
                .title
                .as_deref()
                .and_then(normalized_block_section_title)
            {
                instruction_paragraphs.push(format!("{title}:"));
            }
        }
        instruction_paragraphs.extend(block_paragraphs);
    }

    if instruction_paragraphs.is_empty() {
        return None;
    }

    // Extract servings from chunks near the title (between title and first ingredient)
    let mut servings: Option<String> = None;
    for i in (0..first_ingredient_idx).rev() {
        let chunk = chunks[i].trim();
        if chunk.is_empty() {
            continue;
        }
        let text = HTML_TAG_REGEX.replace_all(chunk, "");
        let text = text.trim().to_lowercase();
        if text.starts_with("makes ") || text.starts_with("serves ") || text.starts_with("yield") {
            servings = Some(decode_html_entities(text.trim()));
            break;
        }
        // Only look back a couple chunks from ingredients
        if first_ingredient_idx - i > 3 {
            break;
        }
    }

    let image_urls = extract_og_image_fast(html).into_iter().collect();
    let source_name = extract_source_name(source_url);

    let ingredients_str = ingredient_lines.join("\n");
    let footnotes = if ingredients_str.contains('*') {
        extract_footnotes_from_html(html)
    } else {
        None
    };

    Some(RawRecipe {
        title,
        description: None,
        ingredients: ingredients_str,
        instructions: instruction_paragraphs.join("\n\n"),
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

static ENTRY_CONTENT_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".entry-content").expect("entry content selector"));

pub(super) fn extract_smittenkitchen_post_instructions(
    document: &Html,
    source_url: &str,
    title: Option<&str>,
    ingredients: Option<&str>,
) -> Option<String> {
    let host = url::Url::parse(source_url).ok()?.host_str()?.to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    if host != "smittenkitchen.com" {
        return None;
    }

    let title = title?;
    let expected_ingredients: Vec<String> = ingredients?
        .lines()
        .map(normalize_smitten_post_text)
        .filter(|line| !line.is_empty() && !line.ends_with(':'))
        .collect();
    if expected_ingredients.is_empty() {
        return None;
    }

    for entry in document.select(&ENTRY_CONTENT_SELECTOR) {
        if let Some(instructions) =
            extract_smittenkitchen_entry_instructions(entry, title, &expected_ingredients)
        {
            return Some(instructions);
        }
    }

    None
}

fn extract_smittenkitchen_entry_instructions(
    entry: ElementRef<'_>,
    title: &str,
    expected_ingredients: &[String],
) -> Option<String> {
    let normalized_title = normalize_smitten_post_text(title).to_lowercase();
    if normalized_title.is_empty() {
        return None;
    }

    let mut saw_title = false;
    let mut saw_ingredient_list = false;
    let mut paragraphs = Vec::new();

    for child in entry.children() {
        let Some(el) = ElementRef::wrap(child) else {
            continue;
        };
        if smitten_post_boundary(&el) {
            if saw_ingredient_list {
                break;
            }
            continue;
        }

        let tag = el.value().name();
        let text = normalize_smitten_post_text(&el.text().collect::<Vec<_>>().join(" "));

        if !saw_title {
            if text.to_lowercase().starts_with(&normalized_title) {
                saw_title = true;
            }
            continue;
        }

        if !saw_ingredient_list {
            if matches!(tag, "ul" | "ol")
                && smitten_list_matches_expected_ingredients(el, expected_ingredients)
            {
                saw_ingredient_list = true;
            }
            continue;
        }

        if tag == "p" {
            if text.is_empty() {
                continue;
            }
            paragraphs.push(text);
        }
    }

    if paragraphs.is_empty() {
        None
    } else {
        Some(paragraphs.join("\n\n"))
    }
}

fn smitten_post_boundary(el: &ElementRef<'_>) -> bool {
    let class_attr = el.value().attr("class").unwrap_or("");
    class_attr.contains("sharedaddy")
        || class_attr.contains("sd-sharing")
        || class_attr.contains("jp-relatedposts")
        || class_attr.contains("sk-recipe-btns")
}

fn smitten_list_matches_expected_ingredients(
    list_el: ElementRef<'_>,
    expected_ingredients: &[String],
) -> bool {
    let mut matches = 0usize;
    let mut items = 0usize;

    for li in list_el.select(&LI_SELECTOR) {
        let item_text = normalize_smitten_post_text(&li.text().collect::<Vec<_>>().join(" "));
        if item_text.is_empty() {
            continue;
        }
        items += 1;
        if expected_ingredients
            .iter()
            .any(|expected| item_text == *expected || item_text.contains(expected))
        {
            matches += 1;
        }
    }

    let required = expected_ingredients.len().min(2);
    items > 0 && matches >= required && matches * 2 >= expected_ingredients.len()
}

fn normalize_smitten_post_text(text: &str) -> String {
    decode_html_entities(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Collect text from an element, skipping the contents of `<del>` and `<s>`
/// subtrees so struck-through ingredients (e.g. `<li><del>2 Tbsp sugar</del></li>`)
/// don't end up in the recipe.
pub(super) fn collect_text_skipping_struck(el: ElementRef<'_>) -> String {
    let mut out = String::new();
    for descendant in el.descendants() {
        if let Some(text) = descendant.value().as_text() {
            let mut in_struck = false;
            let mut ancestor = descendant.parent();
            while let Some(node) = ancestor {
                if let Some(elem) = node.value().as_element() {
                    let name = elem.name();
                    if name == "del" || name == "s" || name == "strike" {
                        in_struck = true;
                        break;
                    }
                }
                ancestor = node.parent();
            }
            if !in_struck {
                out.push_str(text);
            }
        }
    }
    out
}

static HEADLINE_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("h1.headline").expect("headline selector"));

static POST_CONTENT_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.post_content").expect("post content selector"));

static STRONG_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("strong").expect("strong selector"));

/// Site-specific extractor for virtualweberbullet.com recipe pages.
///
/// These pages are hand-authored HTML without Recipe JSON-LD. They share a
/// consistent layout: title in `h1.headline`, content in `div.post_content`,
/// sections delimited by `<h2>` headers, ingredient lists marked by
/// `<p><strong>Section Name</strong></p>` followed immediately by `<ul>`, and
/// instructions as `<p>` paragraphs grouped under their `<h2>` section header.
pub(super) fn extract_recipe_from_virtualweberbullet(
    html: &str,
    document: &Html,
    source_url: &str,
) -> Option<RawRecipe> {
    let host = url::Url::parse(source_url).ok()?.host_str()?.to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    if host != "virtualweberbullet.com" {
        return None;
    }

    let title_el = document.select(&HEADLINE_SELECTOR).next()?;
    let raw_title: String = title_el.text().collect();
    let title = decode_html_entities(raw_title.trim());
    if title.is_empty() {
        return None;
    }

    let post = document.select(&POST_CONTENT_SELECTOR).next()?;

    #[derive(PartialEq)]
    enum State {
        BeforeSummary,
        InSummary,
        InDescription,
        InInstructions,
    }

    let mut description_paragraphs: Vec<String> = Vec::new();
    let mut ingredient_lines: Vec<String> = Vec::new();
    let mut instruction_sections: Vec<(Option<String>, Vec<String>)> = Vec::new();
    let mut current_section: Option<String> = None;
    let mut current_section_paras: Vec<String> = Vec::new();
    let mut state = State::BeforeSummary;
    // A `<p><strong>X</strong></p>` followed by `<ul>` is the site's ingredient
    // list pattern; `pending_strong_text` holds X until we see the next element
    // and decide whether it's an ingredient header or a bold note paragraph.
    let mut pending_strong_text: Option<String> = None;

    // Restore a pending strong-only paragraph as plain prose. Used whenever the
    // next element is something other than the `<ul>` that would consume it as
    // an ingredient header — e.g. a bold "Note:" paragraph mid-instructions.
    macro_rules! flush_pending_strong {
        () => {
            if let Some(text) = pending_strong_text.take() {
                match state {
                    State::BeforeSummary | State::InSummary | State::InDescription => {
                        description_paragraphs.push(text);
                    }
                    State::InInstructions => {
                        current_section_paras.push(text);
                    }
                }
            }
        };
    }

    for child in post.children() {
        let el = match ElementRef::wrap(child) {
            Some(e) => e,
            None => continue,
        };
        let tag = el.value().name();
        match tag {
            "h2" => {
                flush_pending_strong!();
                let raw: String = el.text().collect();
                let h_text = decode_html_entities(raw.trim());
                let lower = h_text.to_lowercase();

                // Stop at post-recipe sections that follow the actual cooking
                // instructions: footer link blocks, author bios, "learn more"
                // pointers, and bonus interview/video content.
                if lower.contains("links on tvwb")
                    || lower.starts_with("about ")
                    || lower.starts_with("learn more")
                    || lower.contains("interview")
                {
                    break;
                }

                if h_text.eq_ignore_ascii_case("Summary") {
                    state = State::InSummary;
                    continue;
                }

                if !current_section_paras.is_empty() {
                    instruction_sections.push((
                        current_section.take(),
                        std::mem::take(&mut current_section_paras),
                    ));
                }
                current_section = Some(h_text);
                state = State::InInstructions;
            }
            "ul" => {
                if state == State::InSummary {
                    flush_pending_strong!();
                    state = State::InDescription;
                    continue;
                }
                if let Some(header) = pending_strong_text.take() {
                    let mut header_emitted = false;
                    for li in el.select(&LI_SELECTOR) {
                        let raw_li = collect_text_skipping_struck(li);
                        let Some(text) = sanitize_extracted_ingredient(&raw_li) else {
                            continue;
                        };
                        if !header_emitted {
                            ingredient_lines.push(format!("{}:", header));
                            header_emitted = true;
                        }
                        ingredient_lines.push(text);
                    }
                } else if state == State::InInstructions {
                    for li in el.select(&LI_SELECTOR) {
                        let raw_li: String = li.text().collect();
                        let text = decode_html_entities(raw_li.trim());
                        if !text.is_empty() {
                            current_section_paras.push(text);
                        }
                    }
                }
            }
            "p" => {
                let inner = el.inner_html();
                let stripped = HTML_TAG_REGEX.replace_all(&inner, "");
                let text = decode_html_entities(stripped.trim());
                if text.is_empty() {
                    flush_pending_strong!();
                    continue;
                }

                let strong_text = el.select(&STRONG_SELECTOR).next().map(|s| {
                    let raw: String = s.text().collect();
                    decode_html_entities(raw.trim())
                });
                if let Some(stext) = strong_text.as_ref() {
                    if !stext.is_empty() && stext == &text {
                        // A new strong-only paragraph supersedes the previous
                        // candidate; flush the old one as prose first.
                        flush_pending_strong!();
                        pending_strong_text = Some(stext.clone());
                        continue;
                    }
                }
                flush_pending_strong!();

                let lower = text.to_lowercase();
                if lower.starts_with("learn more later")
                    || lower.starts_with("notice:")
                    || lower == "back to cooking topics"
                    || lower == "."
                    || lower.contains("adsbygoogle")
                {
                    continue;
                }

                match state {
                    State::BeforeSummary | State::InSummary | State::InDescription => {
                        description_paragraphs.push(text);
                        state = State::InDescription;
                    }
                    State::InInstructions => {
                        current_section_paras.push(text);
                    }
                }
            }
            _ => {
                flush_pending_strong!();
            }
        }
    }
    flush_pending_strong!();

    if !current_section_paras.is_empty() {
        instruction_sections.push((current_section, current_section_paras));
    }

    if ingredient_lines.is_empty() || instruction_sections.is_empty() {
        return None;
    }

    let mut instructions_out: Vec<String> = Vec::new();
    for (section, paras) in instruction_sections {
        if paras.is_empty() {
            continue;
        }
        if let Some(s) = section {
            instructions_out.push(format!("{}:", s));
        }
        for p in paras {
            instructions_out.push(p);
        }
    }

    if instructions_out.is_empty() {
        return None;
    }

    let description = if description_paragraphs.is_empty() {
        None
    } else {
        Some(description_paragraphs.join("\n\n"))
    };

    let mut image_urls: Vec<String> = extract_og_image_fast(html).into_iter().collect();
    if image_urls.is_empty() {
        if let Some(img) = extract_og_image(document) {
            image_urls.push(img);
        }
    }

    Some(RawRecipe {
        title,
        description,
        ingredients: ingredient_lines.join("\n"),
        instructions: instructions_out.join("\n\n"),
        image_urls,
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

/// Regex to detect ingredient-like quantity patterns at the start of a line.
pub(super) static INGREDIENT_QUANTITY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(\d|½|⅓|¼|⅔|¾|⅛|a\s+(pinch|few|handful|dash|splash)|juice\s+of|zest\s+of|pinch\s+of|dash\s+of|kosher\s+salt|salt[,\s]|ground\s|fresh\s|sea\s+salt)")
        .expect("Invalid ingredient quantity regex")
});

/// Check whether an HTML chunk looks like an ingredient list.
/// Ingredient paragraphs have multiple short lines (split by `<br>`) where
/// at least some lines contain quantity-like patterns (digits, fractions)
/// and lines are generally short (not prose paragraphs).
pub(super) fn looks_like_ingredient_list(chunk: &str) -> bool {
    let lines: Vec<&str> = BR_TAG_REGEX.split(chunk).collect();
    if lines.len() < 2 {
        return false;
    }

    let mut quantity_lines = 0;
    let mut total_text_lines = 0;
    let mut long_lines = 0;

    for line in &lines {
        let text = HTML_TAG_REGEX.replace_all(line, "");
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        total_text_lines += 1;

        if text.len() > 200 {
            long_lines += 1;
        }

        // A line looks like an ingredient if it matches quantity patterns
        if text.len() < 300 && INGREDIENT_QUANTITY_REGEX.is_match(text) {
            quantity_lines += 1;
        }
    }

    // Reject chunks where most lines are very long (likely prose, not ingredients)
    if total_text_lines > 0 && (long_lines * 100 / total_text_lines) > 50 {
        return false;
    }

    // At least 2 text lines and at least 40% look like quantities
    total_text_lines >= 2 && quantity_lines > 0 && (quantity_lines * 100 / total_text_lines) >= 40
}

/// Extract individual ingredient lines from a `<br>`-delimited HTML chunk.
///
/// Treats a leading `<u>…</u>` that stands alone (no ingredient text on the
/// same line) as the group's section header and colon-terminates it so the
/// ingredient parser picks it up. Underlines elsewhere in the chunk are
/// stripped as plain inline emphasis — blogs sometimes use `<u>` to highlight
/// a single ingredient, and turning that into a section header would drop the
/// ingredient from the recipe.
pub(super) fn extract_ingredient_lines_from_chunk(chunk: &str, lines: &mut Vec<String>) {
    let mut seen_text_line = false;
    for part in BR_TAG_REGEX.split(chunk) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let u_cap = UNDERLINE_TEXT_REGEX.captures(part);

        if let Some(cap) = u_cap {
            let header = cap.get(1).unwrap().as_str().trim();
            let after_u = UNDERLINE_TEXT_REGEX.replace(part, "");
            let after_text = HTML_TAG_REGEX.replace_all(&after_u, "");
            let after_text = after_text.trim();

            // A section header is a `<u>…</u>` that (a) leads the chunk
            // before any actual text line and (b) owns its line entirely.
            // Empty/markup-only fragments before it (e.g. stray `<span>` or
            // `&nbsp;`) don't disqualify it — that's why we gate on whether
            // we've produced a text line yet, not the raw BR index.
            // If there's non-empty text after the `</u>`, only keep treating
            // it as a header when the suffix is just a trailing qualifier
            // like "(enough for 9 tarts)".
            if !seen_text_line && !header.is_empty() {
                let decoded = decode_html_entities(header);
                let section_title = if after_text.is_empty() {
                    underlined_section_title(&decoded)
                } else {
                    extract_underlined_section_title(part, &decoded)
                };

                if let Some(section_title) = section_title {
                    if section_title.ends_with(':') {
                        lines.push(section_title);
                    } else {
                        lines.push(format!("{section_title}:"));
                    }
                    seen_text_line = true;
                    continue;
                }
            }

            // Inline emphasis or non-leading <u>: keep as a plain ingredient
            // line.
            let text = HTML_TAG_REGEX.replace_all(part, "");
            let text = text.trim();
            if !text.is_empty() {
                lines.push(decode_html_entities(text));
                seen_text_line = true;
            }
        } else {
            let text = HTML_TAG_REGEX.replace_all(part, "");
            let text = text.trim();
            if !text.is_empty() {
                lines.push(decode_html_entities(text));
                seen_text_line = true;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::print_stdout, clippy::print_stderr)]
mod tests {
    use super::*;

    #[test]
    fn test_unstructured_blog_basic_recipe() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <meta property="og:image" content="https://example.com/recipe.jpg">
            </head>
            <body>
                <h1 class="entry-title">My Blog Post About Cake</h1>
                <div class="entry-content">
                    <p>I love making this cake. It reminds me of childhood.</p>
                    <p><strong>Simple Vanilla Cake</strong></p>
                    <p>2 cups flour<br />1 cup sugar<br />3 eggs<br />1 cup milk<br />1 teaspoon vanilla extract</p>
                    <p>Preheat oven to 350. Mix dry ingredients. Add wet ingredients. Pour into pan and bake for 30 minutes.</p>
                </div>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.com/blog/cake").unwrap();
        assert_eq!(result.title, "Simple Vanilla Cake");
        assert!(result.ingredients.contains("2 cups flour"));
        assert!(result.ingredients.contains("3 eggs"));
        assert!(result.ingredients.contains("1 teaspoon vanilla extract"));
        assert!(result.instructions.contains("Preheat oven to 350"));
    }

    #[test]
    fn test_unstructured_blog_with_section_headers() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <p><b>Apple Pie</b></p>
                <p><u>For the crust</u><br />2 cups flour<br />1 stick butter<br />1/4 cup ice water</p>
                <p><u>For the filling</u><br />6 apples<br />1 cup sugar<br />1 teaspoon cinnamon</p>
                <p>Make the crust by cutting butter into flour. Roll out and place in pie dish.</p>
                <p>Slice apples and toss with sugar and cinnamon. Fill the crust and bake at 375 for 45 minutes.</p>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.com/pie").unwrap();
        assert_eq!(result.title, "Apple Pie");
        assert!(result.ingredients.contains("For the crust:"));
        assert!(result.ingredients.contains("2 cups flour"));
        assert!(result.ingredients.contains("For the filling:"));
        assert!(result.ingredients.contains("6 apples"));
        assert!(result.instructions.contains("Make the crust"));
    }

    #[test]
    fn test_unstructured_blog_u_headers_are_colon_terminated() {
        // `<u>` section headers must be emitted colon-terminated so the
        // ingredient parser can detect them as section markers. Otherwise
        // plain headers like "For the chicken" or "To assemble" (no colon
        // in the source HTML) get treated as ingredients.
        let html = r#"
            <html><body>
                <p><b>Fajitas</b></p>
                <p><u>For the chicken</u><br />1 pound chicken<br />1 teaspoon salt</p>
                <p><u>To assemble</u><br />8 tortillas<br />Olive oil<br />2 bell peppers</p>
                <p>Cook everything together until done.</p>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/fajitas").unwrap();
        assert!(
            result.ingredients.contains("For the chicken:"),
            "expected colon-terminated 'For the chicken:' header, got:\n{}",
            result.ingredients
        );
        assert!(
            result.ingredients.contains("To assemble:"),
            "expected colon-terminated 'To assemble:' header, got:\n{}",
            result.ingredients
        );
    }

    #[test]
    fn test_unstructured_blog_u_header_after_markup_only_prefix() {
        // A leading markup-only fragment (empty span, stray &nbsp;, etc.)
        // must not disqualify the following <u>…</u> from being the
        // section header.
        let html = r#"
            <html><body>
                <p><b>Recipe</b></p>
                <p><span></span><br /><u>For the sauce</u><br />1 cup cream<br />1 teaspoon salt</p>
                <p>Whisk everything together until combined and smooth.</p>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/r").unwrap();
        assert!(
            result.ingredients.contains("For the sauce:"),
            "expected colon-terminated header after markup-only prefix, got:\n{}",
            result.ingredients
        );
    }

    #[test]
    fn test_unstructured_blog_u_midline_not_header() {
        // An ingredient line with an underlined word ("emphasis") must NOT be
        // promoted to a section header — otherwise the ingredient disappears.
        let html = r#"
            <html><body>
                <p><b>Recipe</b></p>
                <p>1 pound pasta<br /><u>Olive oil</u><br />1 teaspoon salt<br />1 clove garlic<br />1 cup basil</p>
                <p>Toss everything together and serve immediately while warm.</p>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/r").unwrap();
        assert!(result.ingredients.contains("Olive oil"));
        assert!(
            !result.ingredients.contains("Olive oil:"),
            "<u> emphasis mid-chunk must not become a section header, got:\n{}",
            result.ingredients
        );
    }

    #[test]
    fn test_unstructured_blog_u_header_with_parenthetical_qualifier() {
        let html = r#"
            <html><body>
                <p><b>Homemade Pop Tarts</b></p>
                <p><u>Pastry</u><br />2 cups flour<br />1 egg</p>
                <p><u>Cinnamon Filling</u> (enough for 9 tarts)<br />1/2 cup brown sugar<br />1 teaspoon cinnamon</p>
                <p>Mix and bake.</p>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://smittenkitchen.com/recipe").unwrap();
        let ingredient_lines: Vec<&str> = result.ingredients.lines().collect();

        assert_eq!(ingredient_lines[0], "Pastry:");
        assert_eq!(ingredient_lines[1], "2 cups flour");
        assert_eq!(ingredient_lines[2], "1 egg");
        assert_eq!(ingredient_lines[3], "Cinnamon Filling:");
        assert_eq!(ingredient_lines[4], "1/2 cup brown sugar");
        assert_eq!(ingredient_lines[5], "1 teaspoon cinnamon");
        assert!(!result
            .ingredients
            .contains("Cinnamon Filling (enough for 9 tarts)"));
    }

    #[test]
    fn test_unstructured_blog_u_header_preserves_existing_colon() {
        let html = r#"
            <html><body>
                <p><b>Recipe</b></p>
                <p><u>For the Sauce:</u><br />1 cup cream<br />1 teaspoon salt</p>
                <p>Whisk everything together until combined.</p>
            </body></html>
        "#;

        let result = extract_recipe(html, "https://example.com/r").unwrap();
        assert!(
            result.ingredients.contains("For the Sauce:\n1 cup cream"),
            "expected a single colon in the emitted header, got:\n{}",
            result.ingredients
        );
        assert!(!result.ingredients.contains("For the Sauce::"));
    }

    #[test]
    fn test_underlined_section_title_returns_normalized_name() {
        assert_eq!(
            underlined_section_title("For the Sauce:"),
            Some("For the Sauce".to_string())
        );
        assert_eq!(
            underlined_section_title("Cinnamon Filling (enough for 9 tarts)"),
            Some("Cinnamon Filling".to_string())
        );
    }

    #[test]
    fn test_unstructured_blog_no_ingredients_returns_none() {
        // A blog post without br-delimited ingredient lists should not extract
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <p><b>My Trip to Paris</b></p>
                <p>We visited the Eiffel Tower and ate at a lovely bistro.</p>
                <p>The food was amazing and the views were spectacular.</p>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.com/paris");
        assert!(result.is_err());
    }

    #[test]
    fn test_unstructured_blog_keeps_multiple_recipe_blocks_separate() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <p><b>Yogurt-Marinated Lamb Kebabs</b><br />Adapted from Someone</p>
                <p>1 pound plain yogurt<br />
                1/4 cup olive oil<br />
                2 pounds lamb<br />
                1 red onion</p>
                <p>Combine the yogurt, oil, and lamb in a bowl.</p>
                <p>Grill the skewers until the lamb is medium-rare.</p>
                <p><b>Tzatziki</b><br />From Somewhere Else</p>
                <p>14 ounces Greek yogurt<br />
                1 hothouse cucumber<br />
                1/4 cup sour cream<br />
                2 tablespoons lemon juice</p>
                <p>Place the yogurt in a medium bowl and stir in the cucumber.</p>
                <div class="sharedaddy">share buttons</div>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.com/recipe").unwrap();
        let ingredient_lines: Vec<&str> = result.ingredients.lines().collect();
        assert_eq!(result.title, "Yogurt-Marinated Lamb Kebabs");
        assert_eq!(ingredient_lines[0], "1 pound plain yogurt");
        assert_eq!(ingredient_lines[4], "Tzatziki:");
        assert_eq!(ingredient_lines[5], "14 ounces Greek yogurt");
        assert!(result
            .instructions
            .contains("Combine the yogurt, oil, and lamb in a bowl."));
        assert!(result
            .instructions
            .contains("Grill the skewers until the lamb is medium-rare."));
        assert!(result.instructions.contains("Tzatziki:"));
        assert!(result
            .instructions
            .contains("Place the yogurt in a medium bowl"));
    }

    #[test]
    fn test_unstructured_blog_normalizes_later_block_titles_for_sections() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <p><b>Indian Spiced Cauliflower and Potatoes [Aloo Gobi]</b></p>
                <p>1 head cauliflower<br />
                1 pound potatoes<br />
                5 tablespoons oil</p>
                <p>Roast the vegetables until tender.</p>
                <p><b>Red Split Lentils With Cabbage (Masoor dal aur band gobi)</b></p>
                <p>1 1/4 cups red split lentils<br />
                5 cups water<br />
                1 teaspoon cumin seeds</p>
                <p>Simmer the lentils until soft.</p>
            </body>
            </html>
        "#;

        let result = extract_recipe(html, "https://example.com/aloo-gobi").unwrap();
        let ingredient_lines: Vec<&str> = result.ingredients.lines().collect();
        assert_eq!(ingredient_lines[0], "1 head cauliflower");
        assert_eq!(ingredient_lines[3], "Red Split Lentils with Cabbage:");
        assert_eq!(ingredient_lines[4], "1 1/4 cups red split lentils");
        assert!(!result
            .ingredients
            .contains("Red Split Lentils With Cabbage (Masoor dal aur band gobi):"));
        assert!(result
            .instructions
            .contains("Red Split Lentils with Cabbage:"));
        assert!(!result
            .instructions
            .contains("Red Split Lentils With Cabbage (Masoor dal aur band gobi):"));
    }

    /// Pre-schema Smitten Kitchen posts that are pure narrative (no formal
    /// ingredient list anywhere in the HTML) should fail with a clear
    /// `recipeIngredient (empty)` error rather than panicking, looping, or
    /// inventing ingredients. The 2008 huevos-rancheros post is the
    /// canonical example: it has Jetpack `hrecipe` markup with an empty
    /// `.jetpack-recipe-content` block and prose-only directions.
    #[test]
    fn old_smitten_kitchen_prose_only_post_fails_cleanly() {
        let path = format!(
            "{}/../tests/scrape_fixtures/smittenkitchen/huevos_rancheros.html",
            env!("CARGO_MANIFEST_DIR"),
        );
        let html = std::fs::read_to_string(&path).expect("fixture exists");
        let err = extract_recipe(
            &html,
            "https://smittenkitchen.com/2008/07/huevos-rancheros/",
        )
        .expect_err("prose-only old SK post should not produce a recipe");
        assert!(
            err.to_string().contains("recipeIngredient"),
            "expected ingredient-missing error, got: {err}",
        );
    }

    /// Lookback link sections like "Six Months Ago" / "1.5 Years Ago" must
    /// not be confused for ingredient lists by the unstructured-blog
    /// fallback. The leading digits in "1.5" used to satisfy the quantity
    /// regex, which polluted ingredient extraction and starved the
    /// instruction extractor on real recipes (e.g. crispy peach cobbler).
    #[test]
    fn lookback_links_chunk_is_not_ingredient_list() {
        let chunk = r#"<i>And for the other side of the world:</i><br />
<b>Six Months Ago:</b> <a href="x">Pecan Sticky Buns</a><br />
<b>1.5 Years Ago:</b> <a href="x">Chocolate Peanut Butter Cheesecake</a><br />
<b>2.5 Years Ago:</b> <a href="x">Fried Egg Sandwich</a>"#;
        assert!(
            looks_like_lookback_links_chunk(chunk),
            "lookback links chunk should be detected"
        );
    }

    /// Lookback link chunks must be skipped during instruction extraction
    /// too. The existing `LOOKBACK_LINK_REGEX` only catches "X year(s) ago"
    /// at the start of the chunk, so chunks introduced by
    /// `<i>And for the other side of the world:</i>` followed by month or
    /// decimal-year lookbacks would otherwise leak into instructions.
    #[test]
    fn crispy_peach_cobbler_instructions_exclude_lookback_links() {
        let path = format!(
            "{}/../tests/scrape_fixtures/smittenkitchen/crispy_peach_cobbler.html",
            env!("CARGO_MANIFEST_DIR"),
        );
        let html = std::fs::read_to_string(&path).expect("fixture exists");
        let recipe = extract_recipe(
            &html,
            "https://smittenkitchen.com/2015/08/crispy-peach-cobbler/",
        )
        .expect("extraction should succeed");
        for needle in ["Six Months Ago", "1.5 Years Ago", "Other side of the world"] {
            assert!(
                !recipe.instructions.contains(needle),
                "instructions should not contain {needle:?}, got: {}",
                recipe.instructions,
            );
        }
    }
}
