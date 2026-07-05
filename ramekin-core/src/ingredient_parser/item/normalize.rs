use std::sync::LazyLock;

use regex::Regex;

use crate::ingredient_parser::unicode_fraction_ascii;

/// Normalize unicode characters to their ASCII equivalents.
/// This handles:
/// - Non-breaking spaces → regular spaces
/// - Unicode fractions (½, ⅓, etc.) → ASCII fractions (1/2, 1/3, etc.)
/// - Unicode fraction slash (⁄) → ASCII slash
/// - Unicode dashes (en-dash, em-dash) → ASCII hyphen
pub(in crate::ingredient_parser) fn normalize_unicode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 10);
    let chars: Vec<char> = s.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        if let Some(frac) = unicode_fraction_ascii(c) {
            if i > 0 && chars[i - 1].is_ascii_digit() {
                result.push(' ');
            }
            result.push_str(frac);
            continue;
        }

        match c {
            // Non-breaking space → regular space
            '\u{a0}' => result.push(' '),

            // En-dash and em-dash → ASCII hyphen
            '–' | '—' => result.push('-'),

            // Fraction slash → ASCII slash
            '⁄' => result.push('/'),

            // All other characters pass through unchanged
            _ => result.push(c),
        }
    }

    result
}

pub(in crate::ingredient_parser) fn strip_leading_list_marker(s: &str) -> String {
    let mut remaining = s.trim_start();
    loop {
        let mut chars = remaining.chars();
        let Some(first) = chars.next() else {
            break;
        };
        if matches!(first, '-' | '+' | '*' | '&') {
            let rest = chars.as_str();
            let rest_first = rest.chars().next();
            let should_strip = matches!(
                rest_first,
                Some(c) if c.is_ascii_digit() || c.is_whitespace() || c == '('
            );
            if should_strip {
                remaining = rest.trim_start();
                continue;
            }
        }
        break;
    }
    remaining.to_string()
}

/// Regex for unit words like "grams" attached to numbers: "450grams" → "450 grams"
static UNIT_WORD_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\d+)(grams?)\b").expect("Invalid unit word regex"));

/// Regex for "g" metric unit followed by other letters: "450gpowdered" → "450g powdered"
static METRIC_G_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\d+g)([a-z])").expect("Invalid metric g regex"));

/// Regex for digit(s) followed by 4+ letters (clearly a word): "1finely" → "1 finely"
static DIGIT_WORD_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\d+)([a-z]{4,})").expect("Invalid digit word regex"));

/// Insert space between digits and letters that are clearly separate words.
/// Handles cases like "1finely" → "1 finely" and "450gpowdered" → "450g powdered"
/// But preserves dimension patterns like "6x6-inch".
pub(in crate::ingredient_parser) fn normalize_digit_letter_spacing(s: &str) -> String {
    let s = UNIT_WORD_REGEX.replace_all(s, "$1 $2");
    let s = METRIC_G_REGEX.replace_all(&s, "$1 $2");
    let s = DIGIT_WORD_REGEX.replace_all(&s, "$1 $2");
    s.into_owned()
}
