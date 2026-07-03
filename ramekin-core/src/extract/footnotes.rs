//! Recipe-notes/footnote extraction from HTML notes sections.

use super::*;

/// Regex to match footnote lines starting with asterisks inside `<li>` or `<p>` tags.
/// Captures: (1) the asterisk marker, (2) the footnote text (may contain inline HTML like <em>).
pub(super) static FOOTNOTE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<(?:li|p)[^>]*>\s*(\*{1,3})\s*(.{10,500}?)</(?:li|p)>")
        .expect("Invalid footnote regex")
});

/// Phrases that indicate a false-positive footnote (nutritional disclaimers, affiliate notices).
pub(super) const FOOTNOTE_FALSE_POSITIVE_PREFIXES: &[&str] = &[
    "percent daily value",
    "daily values",
    "this post may contain affiliate",
    "this post contains affiliate",
    "nutrient information is not available",
];

/// Extract footnotes from HTML recipe notes sections.
///
/// Searches for `<li>` and `<p>` elements starting with `*`, `**`, or `***` inside
/// known recipe card note containers (WPRM, Tasty Recipes, etc.).
/// Returns a list of (marker, text) pairs, or None if no footnotes found.
pub fn extract_footnotes_from_html(html: &str) -> Option<Vec<(String, String)>> {
    // Scope the search to the portion of HTML starting from the first recipe notes
    // container. This avoids matching starred <li>/<p> elements in unrelated parts
    // of the page (sidebars, comments) while handling nested <div> elements that
    // break regex-based container content extraction.
    let search_html = find_notes_section_start(html)?;

    let candidates = FOOTNOTE_REGEX.captures_iter(search_html).map(|cap| {
        let marker = cap.get(1).unwrap().as_str().to_string();
        // Strip inline HTML tags (e.g., <em>, <strong>, <a>) from footnote text
        let raw_text = cap.get(2).unwrap().as_str();
        (marker, fragment_to_text(raw_text))
    });

    collect_footnotes(candidates)
}

/// Extract footnotes from a pre-parsed HTML document.
/// Uses CSS selectors instead of regex to avoid re-parsing the DOM.
pub(super) fn extract_footnotes_from_document(document: &Html) -> Option<Vec<(String, String)>> {
    let notes_selector = Selector::parse(
        ".wprm-recipe-notes-container li, .wprm-recipe-notes-container p, .wprm-recipe-notes li, .wprm-recipe-notes p, .tasty-recipes-notes li, .tasty-recipes-notes p, .tasty-recipe-notes li, .tasty-recipe-notes p",
    ).ok()?;

    let candidates = document.select(&notes_selector).filter_map(|element| {
        let text: String = element.text().collect::<String>();
        let text = text.trim().to_string();

        let marker_len = text.chars().take_while(|&c| c == '*').count();
        if marker_len == 0 || marker_len > 3 {
            return None;
        }

        let marker = "*".repeat(marker_len);
        // Safe: '*' is ASCII so marker_len bytes == marker_len chars
        let footnote_text = text.get(marker_len..)?.trim();
        if footnote_text.len() < 10 {
            return None;
        }

        Some((marker, decode_html_entities(footnote_text)))
    });

    collect_footnotes(candidates)
}

/// Shared logic for deduplicating and filtering footnote candidates.
/// Takes an iterator of (marker, text) pairs and returns the first footnote
/// for each marker level, skipping false positives (nutritional disclaimers, etc.).
pub(super) fn collect_footnotes(
    candidates: impl Iterator<Item = (String, String)>,
) -> Option<Vec<(String, String)>> {
    let mut footnotes: Vec<(String, String)> = Vec::new();
    let mut seen_markers = std::collections::HashSet::new();

    for (marker, text) in candidates {
        let lower = text.to_lowercase();
        if FOOTNOTE_FALSE_POSITIVE_PREFIXES
            .iter()
            .any(|prefix| lower.starts_with(prefix))
        {
            continue;
        }

        // Only keep the first footnote for each marker level
        if !seen_markers.contains(&marker) {
            seen_markers.insert(marker.clone());
            footnotes.push((marker, text));
        }
    }

    if footnotes.is_empty() {
        None
    } else {
        Some(footnotes)
    }
}

/// Regex to find a notes container class in an actual HTML class attribute,
/// ignoring matches inside `<style>` or `<script>` blocks.
pub(super) static NOTES_CONTAINER_ATTR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)class\s*=\s*["'][^"']*(wprm-recipe-notes|tasty-recipes?-notes)[^"']*["']"#)
        .expect("Invalid notes container attr regex")
});

/// Find the start of the recipe notes section in HTML.
/// Returns a slice from the first notes container class attribute onwards,
/// scoping footnote search to avoid matching class names in `<style>`/`<script>` blocks.
pub(super) fn find_notes_section_start(html: &str) -> Option<&str> {
    NOTES_CONTAINER_ATTR_REGEX
        .find(html)
        .and_then(|m| html.get(m.start()..))
}

#[cfg(test)]
#[allow(clippy::print_stdout, clippy::print_stderr)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_footnotes_from_wprm_notes() {
        let html = r#"
            <div class="wprm-recipe-notes-container">
                <ul>
                    <li>*If you only have salted butter that works fine too.</li>
                    <li>**Regular or dutch process cocoa works great in this recipe.</li>
                    <li>***Milk chocolate chips give the crackly top.</li>
                </ul>
            </div>
        "#;

        let footnotes = extract_footnotes_from_html(html).unwrap();
        assert_eq!(footnotes.len(), 3);
        assert_eq!(footnotes[0].0, "*");
        assert!(footnotes[0].1.contains("salted butter"));
        assert_eq!(footnotes[1].0, "**");
        assert!(footnotes[1].1.contains("cocoa"));
        assert_eq!(footnotes[2].0, "***");
        assert!(footnotes[2].1.contains("chocolate chips"));
    }

    #[test]
    fn test_extract_footnotes_skips_false_positives() {
        let html = r#"
            <div class="wprm-recipe-notes-container">
                <p>*Percent Daily Values are based on a 2,000 calorie diet.</p>
                <p>**This post may contain affiliate links for products.</p>
            </div>
        "#;

        let footnotes = extract_footnotes_from_html(html);
        assert!(footnotes.is_none());
    }

    #[test]
    fn test_extract_footnotes_none_without_notes_section() {
        let html = r#"
            <div class="recipe-content">
                <p>*This is just a blog paragraph with an asterisk.</p>
            </div>
        "#;

        let footnotes = extract_footnotes_from_html(html);
        assert!(footnotes.is_none());
    }
}
