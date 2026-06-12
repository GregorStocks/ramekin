//! Open Graph image and meta extraction.

use super::*;

/// Regex to find og:image meta tag
pub(super) static OG_IMAGE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<meta[^>]*property\s*=\s*["']og:image["'][^>]*content\s*=\s*["']([^"']+)["'][^>]*/?\s*>"#)
        .expect("Invalid og:image regex")
});

/// Alternative og:image regex (content before property)
pub(super) static OG_IMAGE_REGEX_ALT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<meta[^>]*content\s*=\s*["']([^"']+)["'][^>]*property\s*=\s*["']og:image["'][^>]*/?\s*>"#)
        .expect("Invalid og:image alt regex")
});

/// Fast og:image extraction using regex.
pub(super) fn extract_og_image_fast(html: &str) -> Option<String> {
    // Try property-first pattern
    if let Some(cap) = OG_IMAGE_REGEX.captures(html) {
        return cap.get(1).map(|m| decode_html_entities(m.as_str()));
    }
    // Try content-first pattern
    if let Some(cap) = OG_IMAGE_REGEX_ALT.captures(html) {
        return cap.get(1).map(|m| decode_html_entities(m.as_str()));
    }
    None
}

pub(super) static OG_IMAGE_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"meta[property="og:image"]"#).expect("og:image selector"));

pub(super) static OG_DESCRIPTION_SELECTOR: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(r#"meta[property="og:description"]"#).expect("og:description selector")
});

/// Read an `og:<name>` meta tag's `content` attribute.
pub(super) fn extract_og_meta(document: &Html, selector: &Selector) -> Option<String> {
    document
        .select(selector)
        .next()?
        .value()
        .attr("content")
        .map(decode_html_entities)
}

/// Extract image URL from og:image meta tag.
/// This is a fallback for sites that don't include image data in their recipe structured data
/// (e.g., smittenkitchen.com uses Jetpack recipes which omit itemprop="image").
pub(super) fn extract_og_image(document: &Html) -> Option<String> {
    extract_og_meta(document, &OG_IMAGE_SELECTOR)
}

#[cfg(test)]
#[allow(clippy::print_stdout, clippy::print_stderr)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_og_image() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <meta property="og:image" content="https://example.com/image.jpg">
            </head>
            <body></body>
            </html>
        "#;

        let document = Html::parse_document(html);
        let og_image = extract_og_image(&document);

        assert_eq!(og_image, Some("https://example.com/image.jpg".to_string()));
    }

    #[test]
    fn test_extract_og_image_decodes_html_entities() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <meta property="og:image" content="https://example.com/image.jpg?fit=500%2C333&#038;ssl=1">
            </head>
            <body></body>
            </html>
        "#;

        let document = Html::parse_document(html);
        let og_image = extract_og_image(&document);

        assert_eq!(
            og_image,
            Some("https://example.com/image.jpg?fit=500%2C333&ssl=1".to_string())
        );
    }

    #[test]
    fn test_extract_og_image_fast_decodes_html_entities() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <meta content="https://example.com/image.jpg?fit=500%2C333&#038;ssl=1" property="og:image">
            </head>
            <body></body>
            </html>
        "#;

        let og_image = extract_og_image_fast(html);

        assert_eq!(
            og_image,
            Some("https://example.com/image.jpg?fit=500%2C333&ssl=1".to_string())
        );
    }
}
