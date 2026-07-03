//! Shared helpers for turning HTML fragments and entity-encoded strings into
//! clean plain text.

use regex::Regex;
use std::sync::LazyLock;

/// Regex to strip HTML tags for extracting text from raw HTML fragments.
static HTML_TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<[^>]+>").expect("Invalid HTML tag regex"));

/// Decode HTML entities using the html-escape crate.
/// Also handles double-encoded entities like "&amp;#8531;" by decoding twice.
pub(crate) fn decode_html_entities(text: &str) -> String {
    // First pass: decode entities (this handles &amp; -> & among others)
    let decoded = html_escape::decode_html_entities(text);
    // Second pass: decode again to handle double-encoded entities
    // e.g., "&amp;#8531;" -> "&#8531;" -> "⅓"
    let decoded = html_escape::decode_html_entities(&decoded);
    decoded.into_owned()
}

/// Convert an HTML fragment into clean plain text: strip tags, decode
/// entities, collapse horizontal whitespace within each line, and trim
/// leading/trailing blank lines. Newlines are preserved — source HTML often
/// has meaning-bearing literal newlines (e.g. after each `<br>` in an
/// ingredient list), and collapsing them would merge separate lines. Tags are
/// stripped before decoding so that encoded angle brackets (`&lt;b&gt;`)
/// survive as literal text instead of being eaten as markup.
pub(crate) fn fragment_to_text(fragment: &str) -> String {
    let stripped = HTML_TAG_REGEX.replace_all(fragment, "");
    let decoded = decode_html_entities(&stripped);
    let joined = decoded
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n");
    joined.trim_matches('\n').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_html_entities_basic() {
        assert_eq!(decode_html_entities("Mac &amp; Cheese"), "Mac & Cheese");
        assert_eq!(decode_html_entities("&lt;tag&gt;"), "<tag>");
        assert_eq!(decode_html_entities("&quot;hi&quot;"), "\"hi\"");
    }

    #[test]
    fn test_decode_html_entities_double_encoded() {
        assert_eq!(decode_html_entities("&amp;#8531;"), "⅓");
        assert_eq!(decode_html_entities("&amp;amp;"), "&");
    }

    #[test]
    fn test_fragment_to_text_strips_tags_and_decodes() {
        assert_eq!(
            fragment_to_text("<strong>1 &frac12; cups</strong> flour"),
            "1 ½ cups flour"
        );
        assert_eq!(
            fragment_to_text("Bake at 350&deg;F &#8212; don&#8217;t peek&hellip;"),
            "Bake at 350°F — don\u{2019}t peek…"
        );
    }

    #[test]
    fn test_fragment_to_text_collapses_horizontal_whitespace() {
        assert_eq!(
            fragment_to_text("  1 cup \t sugar&nbsp;&nbsp;(packed) "),
            "1 cup sugar (packed)"
        );
    }

    #[test]
    fn test_fragment_to_text_preserves_newlines() {
        // Literal newlines separate lines (e.g. after each <br> in source
        // HTML) and must not be merged.
        assert_eq!(
            fragment_to_text("2 cups shallots<br />\n2 cups canola oil"),
            "2 cups shallots\n2 cups canola oil"
        );
        // Leading/trailing blank lines are trimmed, interior ones kept.
        assert_eq!(
            fragment_to_text("\n  \nfirst\n \nsecond\n\n"),
            "first\n\nsecond"
        );
    }

    #[test]
    fn test_fragment_to_text_encoded_angle_brackets_survive() {
        // &lt;b&gt; is literal text, not markup — stripping happens first.
        assert_eq!(
            fragment_to_text("use &lt;b&gt; sparingly"),
            "use <b> sparingly"
        );
    }

    #[test]
    fn test_fragment_to_text_empty_fragment() {
        assert_eq!(fragment_to_text("<p>  </p>"), "");
    }
}
