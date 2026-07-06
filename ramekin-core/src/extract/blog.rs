//! Unstructured blog-post extraction heuristics (bold/underline headings, chunk scanning).

use super::*;
use crate::ingredient_parser::unicode_fraction_regex_class;

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
        let text = fragment_to_text(line);
        if text.is_empty() {
            continue;
        }
        text_lines += 1;
        if LOOKBACK_LINK_LINE_REGEX.is_match(&text) {
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

struct RecipeTextSection {
    title: Option<String>,
    paragraphs: Vec<String>,
}

fn push_colon_header(lines: &mut Vec<String>, title: &str) {
    lines.push(format!("{title}:"));
}

fn flatten_recipe_text_sections(sections: Vec<RecipeTextSection>) -> Vec<String> {
    let mut lines = Vec::new();
    for section in sections {
        if section.paragraphs.is_empty() {
            continue;
        }
        if let Some(title) = section.title {
            push_colon_header(&mut lines, &title);
        }
        lines.extend(section.paragraphs);
    }
    lines
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

        let text = fragment_to_text(chunk);
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
    let text = fragment_to_text(part);
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
    let search_html = html_before_comments(html);
    let chunks: Vec<&str> = P_TAG_SPLIT_REGEX.split(search_html).collect();

    let ingredient_chunk_indices = find_unstructured_ingredient_chunk_indices(&chunks);
    if ingredient_chunk_indices.is_empty() {
        return None;
    }

    let blocks = collect_unstructured_recipe_blocks(&chunks, &ingredient_chunk_indices);
    let first_block = blocks.first()?;
    let first_ingredient_idx = *first_block.ingredient_chunk_indices.first()?;

    let ingredient_lines = collect_unstructured_ingredient_lines(&chunks, &blocks);
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

    let instruction_paragraphs =
        flatten_recipe_text_sections(collect_unstructured_instruction_sections(&chunks, &blocks));

    if instruction_paragraphs.is_empty() {
        return None;
    }

    let servings = extract_unstructured_servings(&chunks, first_ingredient_idx);

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

fn html_before_comments(html: &str) -> &str {
    // Limit search to before comments section to avoid picking up user comments.
    // These markers are ASCII so the byte position is always a valid char boundary.
    let comments_pos = html
        .find("<div id=\"comments\"")
        .or_else(|| html.find("<section id=\"comments\""))
        .or_else(|| html.find("<ol class=\"commentlist\""))
        .or_else(|| html.find("<div class=\"comments-area\""));
    match comments_pos {
        Some(pos) => html.get(..pos).unwrap_or(html),
        None => html,
    }
}

fn find_unstructured_ingredient_chunk_indices(chunks: &[&str]) -> Vec<usize> {
    chunks
        .iter()
        .enumerate()
        .filter(|(_, chunk)| is_unstructured_ingredient_chunk(chunk))
        .map(|(i, _)| i)
        .collect()
}

fn is_unstructured_ingredient_chunk(chunk: &str) -> bool {
    let trimmed = chunk.trim();
    !trimmed.is_empty()
        && BR_TAG_REGEX.find_iter(trimmed).count() >= 2
        && !looks_like_lookback_links_chunk(trimmed)
        && looks_like_ingredient_list(trimmed)
}

fn collect_unstructured_recipe_blocks(
    chunks: &[&str],
    ingredient_chunk_indices: &[usize],
) -> Vec<UnstructuredRecipeBlock> {
    let mut blocks: Vec<UnstructuredRecipeBlock> = Vec::new();
    let mut scan_start = 0;
    for &ingredient_idx in ingredient_chunk_indices {
        let (block_title, block_title_chunk_idx) =
            find_nearest_unstructured_block_title(chunks, scan_start, ingredient_idx);

        if starts_new_unstructured_block(&blocks, chunks, block_title_chunk_idx) {
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

    blocks
}

fn find_nearest_unstructured_block_title(
    chunks: &[&str],
    scan_start: usize,
    ingredient_idx: usize,
) -> (Option<String>, Option<usize>) {
    for i in (scan_start..ingredient_idx).rev() {
        let chunk = chunks[i].trim();
        if chunk.is_empty() {
            continue;
        }
        if let Some(title) = extract_bold_heading(chunk) {
            return (Some(title), Some(i));
        }
    }
    (None, None)
}

fn starts_new_unstructured_block(
    blocks: &[UnstructuredRecipeBlock],
    chunks: &[&str],
    block_title_chunk_idx: Option<usize>,
) -> bool {
    let Some(block) = blocks.last() else {
        return true;
    };
    let Some(title_chunk_idx) = block_title_chunk_idx else {
        return false;
    };
    let previous_ingredient_idx = *block
        .ingredient_chunk_indices
        .last()
        .expect("unstructured recipe block has at least one ingredient chunk");
    has_instruction_paragraph_between(chunks, previous_ingredient_idx + 1, title_chunk_idx)
}

fn collect_unstructured_ingredient_lines(
    chunks: &[&str],
    blocks: &[UnstructuredRecipeBlock],
) -> Vec<String> {
    let is_multi_block = blocks.len() > 1;
    let mut ingredient_lines = Vec::new();
    for (block_idx, block) in blocks.iter().enumerate() {
        if is_multi_block && block_idx > 0 {
            push_normalized_block_section_header(&mut ingredient_lines, block.title.as_deref());
        }
        for &idx in &block.ingredient_chunk_indices {
            extract_ingredient_lines_from_chunk(chunks[idx], &mut ingredient_lines);
        }
    }
    ingredient_lines
}

fn collect_unstructured_instruction_sections(
    chunks: &[&str],
    blocks: &[UnstructuredRecipeBlock],
) -> Vec<RecipeTextSection> {
    let is_multi_block = blocks.len() > 1;
    let mut sections = Vec::new();

    for (block_idx, block) in blocks.iter().enumerate() {
        let last_ingredient_idx = *block
            .ingredient_chunk_indices
            .last()
            .expect("unstructured recipe block has at least one ingredient chunk");
        let next_block_title_idx = blocks
            .get(block_idx + 1)
            .and_then(|next_block| next_block.title_chunk_idx);
        let paragraphs = collect_unstructured_instruction_paragraphs(
            chunks,
            last_ingredient_idx + 1,
            next_block_title_idx,
        );
        if paragraphs.is_empty() {
            continue;
        }

        let title = if is_multi_block && block_idx > 0 {
            block
                .title
                .as_deref()
                .and_then(normalized_block_section_title)
        } else {
            None
        };
        sections.push(RecipeTextSection { title, paragraphs });
    }

    sections
}

fn collect_unstructured_instruction_paragraphs(
    chunks: &[&str],
    start_idx: usize,
    end_before_idx: Option<usize>,
) -> Vec<String> {
    let mut paragraphs = Vec::new();
    for (idx, chunk) in chunks.iter().enumerate().skip(start_idx) {
        if end_before_idx.is_some_and(|title_idx| idx >= title_idx) {
            break;
        }

        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }

        if chunk.contains("sharedaddy") || chunk.contains("sd-sharing") {
            break;
        }

        let text = fragment_to_text(chunk);
        if text.is_empty() {
            continue;
        }

        if LOOKBACK_LINK_REGEX.is_match(chunk) || looks_like_lookback_links_chunk(chunk) {
            continue;
        }

        if paragraphs.is_empty() && is_source_credit_paragraph(&text) {
            continue;
        }

        paragraphs.push(text);
    }
    paragraphs
}

fn is_source_credit_paragraph(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.starts_with("adapted from")
        || lower.starts_with("from ")
        || lower.starts_with("recipe from")
        || lower.starts_with("source:")
}

fn push_normalized_block_section_header(lines: &mut Vec<String>, title: Option<&str>) {
    if let Some(title) = title.and_then(normalized_block_section_title) {
        push_colon_header(lines, &title);
    }
}

fn extract_unstructured_servings(chunks: &[&str], first_ingredient_idx: usize) -> Option<String> {
    for i in (0..first_ingredient_idx).rev() {
        let chunk = chunks[i].trim();
        if chunk.is_empty() {
            continue;
        }
        let text = fragment_to_text(chunk).to_lowercase();
        if text.starts_with("makes ") || text.starts_with("serves ") || text.starts_with("yield") {
            return Some(text);
        }
        // Only look back a couple chunks from ingredients.
        if first_ingredient_idx - i > 3 {
            break;
        }
    }
    None
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
    let content = collect_virtualweberbullet_content(post)?;
    let instructions_out = flatten_recipe_text_sections(content.instruction_sections);

    if instructions_out.is_empty() {
        return None;
    }

    let description = if content.description_paragraphs.is_empty() {
        None
    } else {
        Some(content.description_paragraphs.join("\n\n"))
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
        ingredients: content.ingredient_lines.join("\n"),
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

#[derive(PartialEq)]
enum VirtualWeberbulletState {
    BeforeSummary,
    InSummary,
    InDescription,
    InInstructions,
}

struct VirtualWeberbulletContent {
    description_paragraphs: Vec<String>,
    ingredient_lines: Vec<String>,
    instruction_sections: Vec<RecipeTextSection>,
}

struct VirtualWeberbulletBuilder {
    description_paragraphs: Vec<String>,
    ingredient_lines: Vec<String>,
    instruction_sections: Vec<RecipeTextSection>,
    current_section: Option<String>,
    current_section_paras: Vec<String>,
    state: VirtualWeberbulletState,
    pending_strong_text: Option<String>,
}

impl VirtualWeberbulletBuilder {
    fn new() -> Self {
        Self {
            description_paragraphs: Vec::new(),
            ingredient_lines: Vec::new(),
            instruction_sections: Vec::new(),
            current_section: None,
            current_section_paras: Vec::new(),
            state: VirtualWeberbulletState::BeforeSummary,
            pending_strong_text: None,
        }
    }

    fn finish(mut self) -> Option<VirtualWeberbulletContent> {
        self.flush_pending_strong();
        self.finish_current_section();
        if self.ingredient_lines.is_empty() || self.instruction_sections.is_empty() {
            return None;
        }
        Some(VirtualWeberbulletContent {
            description_paragraphs: self.description_paragraphs,
            ingredient_lines: self.ingredient_lines,
            instruction_sections: self.instruction_sections,
        })
    }

    fn handle_element(&mut self, el: ElementRef<'_>) -> bool {
        match el.value().name() {
            "h2" => self.handle_h2(el),
            "ul" => {
                self.handle_ul(el);
                true
            }
            "p" => {
                self.handle_paragraph(el);
                true
            }
            _ => {
                self.flush_pending_strong();
                true
            }
        }
    }

    fn handle_h2(&mut self, el: ElementRef<'_>) -> bool {
        self.flush_pending_strong();
        let raw: String = el.text().collect();
        let h_text = decode_html_entities(raw.trim());

        if is_virtualweberbullet_footer_heading(&h_text) {
            return false;
        }

        if h_text.eq_ignore_ascii_case("Summary") {
            self.state = VirtualWeberbulletState::InSummary;
            return true;
        }

        self.finish_current_section();
        self.current_section = Some(h_text);
        self.state = VirtualWeberbulletState::InInstructions;
        true
    }

    fn handle_ul(&mut self, el: ElementRef<'_>) {
        if self.state == VirtualWeberbulletState::InSummary {
            self.flush_pending_strong();
            self.state = VirtualWeberbulletState::InDescription;
            return;
        }
        if let Some(header) = self.pending_strong_text.take() {
            append_virtualweberbullet_ingredient_list(el, &header, &mut self.ingredient_lines);
        } else if self.state == VirtualWeberbulletState::InInstructions {
            append_virtualweberbullet_instruction_list(el, &mut self.current_section_paras);
        }
    }

    fn handle_paragraph(&mut self, el: ElementRef<'_>) {
        let inner = el.inner_html();
        let text = fragment_to_text(&inner);
        if text.is_empty() {
            self.flush_pending_strong();
            return;
        }

        if let Some(strong_text) = virtualweberbullet_strong_only_text(el, &text) {
            self.flush_pending_strong();
            self.pending_strong_text = Some(strong_text);
            return;
        }
        self.flush_pending_strong();

        if is_ignored_virtualweberbullet_paragraph(&text) {
            return;
        }

        self.push_paragraph(text);
    }

    fn flush_pending_strong(&mut self) {
        if let Some(text) = self.pending_strong_text.take() {
            match self.state {
                VirtualWeberbulletState::BeforeSummary
                | VirtualWeberbulletState::InSummary
                | VirtualWeberbulletState::InDescription => {
                    self.description_paragraphs.push(text);
                }
                VirtualWeberbulletState::InInstructions => {
                    self.current_section_paras.push(text);
                }
            }
        }
    }

    fn push_paragraph(&mut self, text: String) {
        match self.state {
            VirtualWeberbulletState::BeforeSummary
            | VirtualWeberbulletState::InSummary
            | VirtualWeberbulletState::InDescription => {
                self.description_paragraphs.push(text);
                self.state = VirtualWeberbulletState::InDescription;
            }
            VirtualWeberbulletState::InInstructions => {
                self.current_section_paras.push(text);
            }
        }
    }

    fn finish_current_section(&mut self) {
        if !self.current_section_paras.is_empty() {
            self.instruction_sections.push(RecipeTextSection {
                title: self.current_section.take(),
                paragraphs: std::mem::take(&mut self.current_section_paras),
            });
        }
    }
}

fn collect_virtualweberbullet_content(post: ElementRef<'_>) -> Option<VirtualWeberbulletContent> {
    let mut builder = VirtualWeberbulletBuilder::new();
    for child in post.children() {
        let Some(el) = ElementRef::wrap(child) else {
            continue;
        };
        if !builder.handle_element(el) {
            break;
        }
    }
    builder.finish()
}

fn is_virtualweberbullet_footer_heading(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("links on tvwb")
        || lower.starts_with("about ")
        || lower.starts_with("learn more")
        || lower.contains("interview")
}

fn append_virtualweberbullet_ingredient_list(
    el: ElementRef<'_>,
    header: &str,
    ingredient_lines: &mut Vec<String>,
) {
    let mut header_emitted = false;
    for li in el.select(&LI_SELECTOR) {
        let raw_li = collect_text_skipping_struck(li);
        let Some(text) = sanitize_extracted_ingredient(&raw_li) else {
            continue;
        };
        if !header_emitted {
            push_colon_header(ingredient_lines, header);
            header_emitted = true;
        }
        ingredient_lines.push(text);
    }
}

fn append_virtualweberbullet_instruction_list(el: ElementRef<'_>, paragraphs: &mut Vec<String>) {
    for li in el.select(&LI_SELECTOR) {
        let raw_li: String = li.text().collect();
        let text = decode_html_entities(raw_li.trim());
        if !text.is_empty() {
            paragraphs.push(text);
        }
    }
}

fn virtualweberbullet_strong_only_text(el: ElementRef<'_>, paragraph_text: &str) -> Option<String> {
    let strong_text = el.select(&STRONG_SELECTOR).next().map(|s| {
        let raw: String = s.text().collect();
        fragment_to_text(&raw)
    })?;
    if !strong_text.is_empty() && strong_text == paragraph_text {
        Some(strong_text)
    } else {
        None
    }
}

fn is_ignored_virtualweberbullet_paragraph(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.starts_with("learn more later")
        || lower.starts_with("notice:")
        || lower == "back to cooking topics"
        || lower == "."
        || lower.contains("adsbygoogle")
}

/// Regex to detect ingredient-like quantity patterns at the start of a line.
pub(super) static INGREDIENT_QUANTITY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)^(\d|[{}]|a\s+(pinch|few|handful|dash|splash)|juice\s+of|zest\s+of|pinch\s+of|dash\s+of|kosher\s+salt|salt[,\s]|ground\s|fresh\s|sea\s+salt)",
        unicode_fraction_regex_class()
    ))
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
        let text = fragment_to_text(line);
        if text.is_empty() {
            continue;
        }
        total_text_lines += 1;

        if text.len() > 200 {
            long_lines += 1;
        }

        // A line looks like an ingredient if it matches quantity patterns
        if text.len() < 300 && INGREDIENT_QUANTITY_REGEX.is_match(&text) {
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

        if let Some(cap) = UNDERLINE_TEXT_REGEX.captures(part) {
            let header = cap.get(1).unwrap().as_str().trim();
            let after_u = UNDERLINE_TEXT_REGEX.replace(part, "");
            let after_text = fragment_to_text(&after_u);

            // A section header is a `<u>…</u>` that (a) leads the chunk
            // before any actual text line and (b) owns its line entirely.
            // Empty/markup-only fragments before it (e.g. stray `<span>` or
            // `&nbsp;`) don't disqualify it — that's why we gate on whether
            // we've produced a text line yet, not the raw BR index.
            // If there's non-empty text after the `</u>`, only keep treating
            // it as a header when the suffix is just a trailing qualifier
            // like "(enough for 9 tarts)".
            if !seen_text_line && !header.is_empty() {
                // Normalize like the fragment_to_text(part) inside
                // extract_underlined_section_title, so strip_prefix isn't
                // broken by &nbsp; or doubled spaces in the header.
                let decoded = fragment_to_text(header);
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
        }

        // Inline emphasis, non-leading <u>, or no <u> at all: keep as a
        // plain ingredient line.
        let text = fragment_to_text(part);
        if !text.is_empty() {
            lines.push(text);
            seen_text_line = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ingredient_quantity_regex_uses_shared_unicode_fraction_set() {
        for fraction in unicode_fraction_regex_class().chars() {
            let line = format!("{fraction} cup sugar");
            assert!(
                INGREDIENT_QUANTITY_REGEX.is_match(&line),
                "expected {fraction} to match ingredient quantity regex"
            );
        }
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
}
