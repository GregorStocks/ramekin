//! Open Graph image and meta extraction.

use super::*;

/// Image downloads require an absolute HTTP(S) URL, not an inline placeholder.
pub(super) fn is_fetchable_image_url(value: &str) -> bool {
    url::Url::parse(value)
        .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}

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
    OG_IMAGE_REGEX
        .captures_iter(html)
        .chain(OG_IMAGE_REGEX_ALT.captures_iter(html))
        .filter_map(|cap| cap.get(1).map(|m| decode_html_entities(m.as_str())))
        .find(|url| is_fetchable_image_url(url))
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
    document
        .select(&OG_IMAGE_SELECTOR)
        .filter_map(|el| el.value().attr("content"))
        .map(decode_html_entities)
        .find(|url| is_fetchable_image_url(url))
}

#[cfg(test)]
#[allow(clippy::print_stdout, clippy::print_stderr)]
mod tests {
    use super::*;

    const PLACEHOLDER: &str = "data:image/svg+xml,%3Csvg viewBox='0 0 150 150'%3E%3C/svg%3E";

    #[test]
    fn image_candidates_require_http() {
        for url in [
            "https://example.com/photo.jpg",
            "http://example.com/photo.png",
        ] {
            assert!(is_fetchable_image_url(url), "{url}");
        }
        for url in [
            PLACEHOLDER,
            "DATA:image/png;base64,AAAA",
            "blob:https://example.com/id",
            "file:///photo.jpg",
            "",
            "/photo.jpg",
        ] {
            assert!(!is_fetchable_image_url(url), "{url}");
        }
    }

    #[test]
    fn jsonld_filters_placeholders_before_og_fallback() {
        let photo = "https://example.com/photo.jpg";
        for (images, expected) in [
            (
                serde_json::json!([PLACEHOLDER, photo, {"url": "http://example.com/second.jpg"}]),
                vec![photo, "http://example.com/second.jpg"],
            ),
            (
                serde_json::json!(PLACEHOLDER),
                vec!["https://example.com/fallback.jpg"],
            ),
            (
                serde_json::json!({"url": PLACEHOLDER}),
                vec!["https://example.com/fallback.jpg"],
            ),
        ] {
            let recipe = serde_json::json!({
                "@type": "Recipe", "name": "Soup", "recipeIngredient": ["1 cup water"],
                "recipeInstructions": "Boil water.", "image": images
            });
            let html = format!(
                r#"<script type="application/ld+json">{recipe}</script><meta property="og:image" content="https://example.com/fallback.jpg">"#
            );
            let document = Html::parse_document(&html);
            assert_eq!(
                extract_jsonld_fast(&html, "https://example.com")
                    .unwrap()
                    .image_urls,
                expected
            );
            assert_eq!(
                extract_recipe_from_jsonld(&document, "https://example.com")
                    .unwrap()
                    .image_urls,
                expected
            );
            assert_eq!(
                extract_recipe_with_stats(&html, "https://example.com")
                    .unwrap()
                    .raw_recipe
                    .image_urls,
                expected
            );
        }
    }

    #[test]
    fn microdata_filters_placeholders_before_og_fallback() {
        for (extra_image, expected) in [
            (
                r#"<img itemprop="image" src="https://example.com/photo.jpg">"#,
                vec!["https://example.com/photo.jpg"],
            ),
            (
                r#"<meta property="og:image" content="https://example.com/fallback.jpg">"#,
                vec!["https://example.com/fallback.jpg"],
            ),
            ("", vec![]),
        ] {
            let html = format!(
                r#"<div itemscope itemtype="https://schema.org/Recipe"><span itemprop="name">Soup</span><span itemprop="recipeIngredient">1 cup water</span><span itemprop="recipeInstructions">Boil water.</span><img itemprop="image" src="{PLACEHOLDER}">{extra_image}</div>"#
            );
            assert_eq!(
                extract_recipe(&html, "https://example.com")
                    .unwrap()
                    .image_urls,
                expected
            );
        }
    }

    #[test]
    fn og_images_skip_placeholders_in_both_attribute_orders() {
        for placeholder in [
            "data:image/svg+xml,%3Csvg/%3E",
            "data:image/png;base64,AAAA",
        ] {
            for tag in [
                format!(r#"<meta property="og:image" content="{placeholder}">"#),
                format!(r#"<meta content="{placeholder}" property="og:image">"#),
            ] {
                assert_eq!(extract_og_image_fast(&tag), None);
                assert_eq!(extract_og_image(&Html::parse_document(&tag)), None);
                let html = format!(
                    r#"{tag}<meta property="og:image" content="https://example.com/photo.jpg">"#
                );
                assert_eq!(
                    extract_og_image_fast(&html).as_deref(),
                    Some("https://example.com/photo.jpg")
                );
                assert_eq!(
                    extract_og_image(&Html::parse_document(&html)).as_deref(),
                    Some("https://example.com/photo.jpg")
                );
            }
        }
    }

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
