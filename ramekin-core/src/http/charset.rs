//! Charset detection and transcoding for HTTP responses.
//!
//! Detects character encoding from Content-Type headers and HTML meta tags,
//! then transcodes to UTF-8 using encoding_rs.

use encoding_rs::Encoding;

/// Decode raw bytes to a UTF-8 string, detecting charset from HTTP headers and HTML meta tags.
///
/// Detection priority:
/// 1. `charset=` parameter from Content-Type header
/// 2. `<meta charset="...">` in first 1024 bytes of HTML
/// 3. `<meta http-equiv="Content-Type" content="...; charset=...">` in first 1024 bytes
/// 4. Direct UTF-8 if bytes are valid UTF-8
/// 5. Lossy UTF-8 conversion (replaces invalid bytes with U+FFFD)
pub fn decode_bytes_to_utf8(bytes: &[u8], content_type: Option<&str>) -> String {
    // Try charset from Content-Type header
    if let Some(ct) = content_type {
        if let Some(encoding) = charset_from_content_type(ct) {
            if encoding != encoding_rs::UTF_8 {
                let (decoded, _, _) = encoding.decode(bytes);
                return decoded.into_owned();
            }
        }
    }

    // Try charset from HTML meta tags (scan first 1024 bytes)
    if let Some(encoding) = charset_from_html_meta(bytes) {
        if encoding != encoding_rs::UTF_8 {
            let (decoded, _, _) = encoding.decode(bytes);
            return decoded.into_owned();
        }
    }

    // Try direct UTF-8
    match String::from_utf8(bytes.to_vec()) {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!("falling back to lossy UTF-8 conversion: {}", e);
            String::from_utf8_lossy(bytes).into_owned()
        }
    }
}

/// Extract charset from a Content-Type header value.
/// e.g. "text/html; charset=iso-8859-1" -> Some(ISO_8859_1)
fn charset_from_content_type(content_type: &str) -> Option<&'static Encoding> {
    // Content-Type headers are always ASCII, so working on bytes is safe
    let lower = content_type.to_ascii_lowercase();
    let charset_value = lower
        .split("charset=")
        .nth(1)?
        .trim_start_matches('"')
        .split(['"', ';', ',', ' '])
        .next()?
        .trim();

    if charset_value.is_empty() {
        return None;
    }

    Encoding::for_label(charset_value.as_bytes())
}

/// Scan first 1024 bytes of HTML for charset declarations in meta tags.
/// Works on raw bytes to find charset, then uses encoding_rs to look up the encoding.
fn charset_from_html_meta(bytes: &[u8]) -> Option<&'static Encoding> {
    let scan_len = bytes.len().min(1024);
    let scan_bytes = &bytes[..scan_len];

    // Search for "charset=" (case-insensitive) in the byte stream
    let charset_pos = find_case_insensitive(scan_bytes, b"charset=")?;

    // Verify we're inside a <meta tag by scanning backward for '<'
    let before = &scan_bytes[..charset_pos];
    let tag_start = memrchr(b'<', before)?;
    let tag_bytes = &scan_bytes[tag_start..charset_pos];

    // Check the tag contains "meta" (case-insensitive)
    if !contains_case_insensitive(tag_bytes, b"meta") {
        return None;
    }

    // Extract the charset value after "charset="
    let value_start = charset_pos + b"charset=".len();
    if value_start >= scan_len {
        return None;
    }
    let rest = &scan_bytes[value_start..];

    // Skip optional quote delimiter and extract until closing quote or delimiter
    let (skip, delimiters): (usize, &[u8]) = match rest.first()? {
        b'"' => (1, b"\"" as &[u8]),
        b'\'' => (1, b"'" as &[u8]),
        _ => (0, b"\"'>; " as &[u8]),
    };

    let value_bytes = &rest[skip..];
    let end = value_bytes
        .iter()
        .position(|b| delimiters.contains(b))
        .unwrap_or(value_bytes.len());
    let charset_name = &value_bytes[..end];

    // Trim ASCII whitespace
    let charset_name = trim_ascii(charset_name);
    if charset_name.is_empty() {
        return None;
    }

    Encoding::for_label(charset_name)
}

/// Find a needle (case-insensitive) in a byte slice, returning the position of the first match.
fn find_case_insensitive(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
}

/// Check if a byte slice contains a needle (case-insensitive).
fn contains_case_insensitive(haystack: &[u8], needle: &[u8]) -> bool {
    find_case_insensitive(haystack, needle).is_some()
}

/// Find the last occurrence of a byte in a slice.
fn memrchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    haystack.iter().rposition(|&b| b == needle)
}

/// Trim leading and trailing ASCII whitespace from a byte slice.
fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map(|p| p + 1)
        .unwrap_or(start);
    &bytes[start..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_type_charset() {
        let enc = charset_from_content_type("text/html; charset=iso-8859-1").unwrap();
        assert_eq!(enc.name(), "windows-1252"); // encoding_rs maps iso-8859-1 to windows-1252
    }

    #[test]
    fn content_type_charset_uppercase() {
        let enc = charset_from_content_type("text/html; CHARSET=UTF-8").unwrap();
        assert_eq!(enc, encoding_rs::UTF_8);
    }

    #[test]
    fn content_type_charset_quoted() {
        let enc = charset_from_content_type("text/html; charset=\"windows-1252\"").unwrap();
        assert_eq!(enc.name(), "windows-1252");
    }

    #[test]
    fn content_type_no_charset() {
        assert!(charset_from_content_type("text/html").is_none());
    }

    #[test]
    fn content_type_empty_charset() {
        assert!(charset_from_content_type("text/html; charset=").is_none());
    }

    #[test]
    fn meta_charset_html5() {
        let html = b"<html><head><meta charset=\"iso-8859-1\"></head>";
        let enc = charset_from_html_meta(html).unwrap();
        assert_eq!(enc.name(), "windows-1252");
    }

    #[test]
    fn meta_charset_single_quotes() {
        let html = b"<html><head><meta charset='utf-8'></head>";
        let enc = charset_from_html_meta(html).unwrap();
        assert_eq!(enc, encoding_rs::UTF_8);
    }

    #[test]
    fn meta_http_equiv() {
        let html =
            b"<html><head><meta http-equiv=\"Content-Type\" content=\"text/html; charset=iso-8859-1\"></head>";
        let enc = charset_from_html_meta(html).unwrap();
        assert_eq!(enc.name(), "windows-1252");
    }

    #[test]
    fn meta_no_charset() {
        let html = b"<html><head><meta name=\"viewport\" content=\"width=device-width\"></head>";
        assert!(charset_from_html_meta(html).is_none());
    }

    #[test]
    fn decode_iso_8859_1_bytes() {
        let bytes = b"caf\xe9";
        let result = decode_bytes_to_utf8(bytes, Some("text/html; charset=iso-8859-1"));
        assert_eq!(result, "café");
    }

    #[test]
    fn decode_windows_1252_smart_quotes() {
        let bytes = b"\x93hello\x94";
        let result = decode_bytes_to_utf8(bytes, Some("text/html; charset=windows-1252"));
        assert_eq!(result, "\u{201c}hello\u{201d}");
    }

    #[test]
    fn decode_utf8_passthrough() {
        let text = "Hello, café! 日本語";
        let result = decode_bytes_to_utf8(text.as_bytes(), None);
        assert_eq!(result, text);
    }

    #[test]
    fn decode_utf8_with_content_type() {
        let text = "Hello, world!";
        let result = decode_bytes_to_utf8(text.as_bytes(), Some("text/html; charset=utf-8"));
        assert_eq!(result, text);
    }

    #[test]
    fn decode_unknown_encoding_falls_back_to_lossy() {
        let bytes = b"hello \xff world";
        let result = decode_bytes_to_utf8(bytes, None);
        assert_eq!(result, "hello \u{FFFD} world");
    }

    #[test]
    fn decode_empty_input() {
        let result = decode_bytes_to_utf8(b"", None);
        assert_eq!(result, "");
    }

    #[test]
    fn decode_charset_from_meta_tag() {
        let html =
            b"<html><head><meta charset=\"iso-8859-1\"></head><body>caf\xe9</body></html>".to_vec();
        let result = decode_bytes_to_utf8(&html, None);
        assert_eq!(
            result,
            "<html><head><meta charset=\"iso-8859-1\"></head><body>café</body></html>"
        );
    }

    #[test]
    fn content_type_takes_precedence_over_meta() {
        let bytes = b"<html><head><meta charset=\"utf-8\"></head><body>caf\xe9</body></html>";
        let result = decode_bytes_to_utf8(bytes, Some("text/html; charset=iso-8859-1"));
        assert_eq!(
            result,
            "<html><head><meta charset=\"utf-8\"></head><body>café</body></html>"
        );
    }
}
