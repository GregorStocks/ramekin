//! Line filtering, prep/guidance notes, section headers, and item splitting.

use super::*;

/// Common preparation notes
pub(super) const PREP_NOTES: &[&str] = &[
    "at room temperature",
    "room temperature",
    "loosely packed",
    "firmly packed",
    "lightly beaten",
    "roughly chopped",
    "coarsely chopped",
    "finely chopped",
    "thinly sliced",
    "plus more for",
    "for garnish",
    "for serving",
    "approximately",
    "julienned",
    "quartered",
    "shredded",
    "blanched",
    "crumbled",
    "softened",
    "uncooked",
    "combined",
    "reserved",
    "cooled",
    "or more",
    "or less",
    "optional",
    "to taste",
    "as needed",
    "chopped",
    "crushed",
    "cleaned",
    "divided",
    "drained",
    "toasted",
    "roasted",
    "trimmed",
    "whisked",
    "chilled",
    "minced",
    "sliced",
    "grated",
    "melted",
    "cooked",
    "ground",
    "beaten",
    "thawed",
    "frozen",
    "peeled",
    "washed",
    "rinsed",
    "packed",
    "sifted",
    "halved",
    "diced",
    "cubed",
    "cored",
    "mixed",
    "fresh",
    "dried",
    "whole",
    "cold",
    "raw",
    "scrubbed",
];

/// Decode HTML entities using the html-escape crate.
/// Also handles double-encoded entities like "&amp;#8531;" by decoding twice.
pub(super) fn decode_html_entities(s: &str) -> String {
    // First pass: decode entities (this handles &amp; -> & among others)
    let decoded = html_escape::decode_html_entities(s);

    // Second pass: decode again to handle double-encoded entities
    // e.g., "&amp;#8531;" -> "&#8531;" -> "⅓"
    let decoded = html_escape::decode_html_entities(&decoded);

    decoded.into_owned()
}

/// Normalize unicode characters to their ASCII equivalents.
/// This handles:
/// - Non-breaking spaces → regular spaces
/// - Unicode fractions (½, ⅓, etc.) → ASCII fractions (1/2, 1/3, etc.)
/// - Unicode fraction slash (⁄) → ASCII slash
/// - Unicode dashes (en-dash, em-dash) → ASCII hyphen
pub(super) fn normalize_unicode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 10);
    let chars: Vec<char> = s.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        match c {
            // Non-breaking space → regular space
            '\u{a0}' => result.push(' '),

            // En-dash and em-dash → ASCII hyphen
            '–' | '—' => result.push('-'),

            // Fraction slash → ASCII slash
            '⁄' => result.push('/'),

            // Unicode fractions → ASCII fractions
            // Add space if preceded by a digit (e.g., "1½" -> "1 1/2")
            '½' | '⅓' | '⅔' | '¼' | '¾' | '⅕' | '⅖' | '⅗' | '⅘' | '⅙' | '⅚' | '⅛' | '⅜' | '⅝'
            | '⅞' => {
                let frac = match c {
                    '½' => "1/2",
                    '⅓' => "1/3",
                    '⅔' => "2/3",
                    '¼' => "1/4",
                    '¾' => "3/4",
                    '⅕' => "1/5",
                    '⅖' => "2/5",
                    '⅗' => "3/5",
                    '⅘' => "4/5",
                    '⅙' => "1/6",
                    '⅚' => "5/6",
                    '⅛' => "1/8",
                    '⅜' => "3/8",
                    '⅝' => "5/8",
                    '⅞' => "7/8",
                    _ => unreachable!(),
                };
                if i > 0 && chars[i - 1].is_ascii_digit() {
                    result.push(' ');
                }
                result.push_str(frac);
            }

            // All other characters pass through unchanged
            _ => result.push(c),
        }
    }

    result
}

pub(super) fn strip_leading_list_marker(s: &str) -> String {
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
static RE_UNIT_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\d+)(grams?)\b").unwrap());

/// Regex for "g" metric unit followed by other letters: "450gpowdered" → "450g powdered"
static RE_METRIC_G: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(\d+g)([a-z])").unwrap());

/// Regex for digit(s) followed by 4+ letters (clearly a word): "1finely" → "1 finely"
static RE_DIGIT_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\d+)([a-z]{4,})").unwrap());

/// Insert space between digits and letters that are clearly separate words.
/// Handles cases like "1finely" → "1 finely" and "450gpowdered" → "450g powdered"
/// But preserves dimension patterns like "6x6-inch".
pub(super) fn normalize_digit_letter_spacing(s: &str) -> String {
    let s = RE_UNIT_WORD.replace_all(s, "$1 $2");
    let s = RE_METRIC_G.replace_all(&s, "$1 $2");
    let s = RE_DIGIT_WORD.replace_all(&s, "$1 $2");
    s.into_owned()
}

/// Check if a string looks like a preparation note.
pub(super) fn is_prep_note(s: &str) -> bool {
    let s_lower = s.to_lowercase();
    PREP_NOTES.iter().any(|note| s_lower.contains(note))
}

pub(super) fn is_trailing_prep_note(s: &str) -> bool {
    if is_strict_trailing_prep_note(s) {
        return true;
    }

    if is_trailing_prep_note_with_context(s) {
        return true;
    }

    if !is_prep_note(s) {
        return false;
    }

    contains_active_prep_note(s) || ambiguous_prep_note_has_allowed_context(s)
}

pub(super) fn is_strict_trailing_prep_note(s: &str) -> bool {
    const PREP_FILLER_WORDS: &[&str] = &[
        "and", "but", "clean", "coarsely", "fine", "finely", "firmly", "freshly", "lightly",
        "loosely", "not", "or", "roughly", "small", "thinly", "to", "very", "well",
    ];

    let mut saw_prep = false;
    for part in s.to_lowercase().split(',') {
        let part = part
            .trim()
            .trim_matches(|c: char| !c.is_ascii_alphabetic() && !c.is_ascii_whitespace());
        if part.is_empty() {
            continue;
        }
        if PREP_NOTES.contains(&part) {
            saw_prep = true;
            continue;
        }

        let words = part
            .split_whitespace()
            .map(|word| word.trim_matches(|c: char| !c.is_ascii_alphabetic()))
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        let mut word_index = 0;
        while word_index < words.len() {
            let word = words[word_index];
            if PREP_FILLER_WORDS.contains(&word) {
                word_index += 1;
                continue;
            }

            if let Some(note_word_count) = PREP_NOTES.iter().find_map(|note| {
                let note_words = note.split_whitespace().collect::<Vec<_>>();
                words
                    .get(word_index..word_index + note_words.len())
                    .is_some_and(|candidate| candidate == note_words)
                    .then_some(note_words.len())
            }) {
                saw_prep = true;
                word_index += note_word_count;
            } else {
                return false;
            }
        }
    }

    saw_prep
}

pub(super) fn is_trailing_prep_note_with_context(s: &str) -> bool {
    let normalized = s
        .trim()
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && !c.is_ascii_whitespace())
        .to_lowercase();
    if normalized.is_empty() {
        return false;
    }

    ACTIVE_PREP_PREFIXES.iter().any(|prefix| {
        normalized
            .strip_prefix(prefix)
            .is_some_and(is_allowed_trailing_prep_context)
    })
}

pub(super) const ACTIVE_PREP_PREFIXES: &[&str] = &[
    "adjusted",
    "bagged",
    "beaten",
    "blanched",
    "chilled",
    "chopped",
    "combined",
    "cooled",
    "cored",
    "crumbled",
    "crushed",
    "cubed",
    "cut",
    "diced",
    "divided",
    "drained",
    "grated",
    "grilled",
    "ground",
    "halved",
    "julienned",
    "melted",
    "minced",
    "mixed",
    "patted dry",
    "peeled",
    "picked over",
    "quartered",
    "removed",
    "reserved",
    "rinsed",
    "roasted",
    "scraped",
    "seeded",
    "shredded",
    "sifted",
    "sliced",
    "softened",
    "stemmed",
    "thawed",
    "toasted",
    "trimmed",
    "unpeeled",
    "washed",
    "well-shaken",
    "whisked",
    "squeezed",
];

pub(super) fn contains_active_prep_note(s: &str) -> bool {
    let normalized = s.to_lowercase();
    ACTIVE_PREP_PREFIXES.iter().any(|prefix| {
        normalized
            .match_indices(prefix)
            .any(|(idx, _)| has_word_boundaries(&normalized, idx, prefix.len()))
    })
}

pub(super) fn has_word_boundaries(s: &str, start: usize, len: usize) -> bool {
    let before = s.get(..start).and_then(|prefix| prefix.chars().next_back());
    let after = s
        .get(start + len..)
        .and_then(|suffix| suffix.chars().next());
    before.is_none_or(|c| !c.is_ascii_alphabetic())
        && after.is_none_or(|c| !c.is_ascii_alphabetic())
}

pub(super) fn ambiguous_prep_note_has_allowed_context(s: &str) -> bool {
    const AMBIGUOUS_PREP_WORDS: &[&str] = &[
        "cold", "cooked", "dried", "fresh", "frozen", "uncooked", "whole",
    ];

    let normalized = s.to_lowercase();
    AMBIGUOUS_PREP_WORDS.iter().any(|word| {
        normalized.match_indices(word).any(|(idx, _)| {
            has_word_boundaries(&normalized, idx, word.len())
                && normalized
                    .get(idx + word.len()..)
                    .is_some_and(is_allowed_ambiguous_prep_context)
        })
    })
}

pub(super) fn is_allowed_ambiguous_prep_context(tail: &str) -> bool {
    let tail = tail.trim_start();
    if tail.is_empty() {
        return true;
    }

    const ALLOWED_CONTEXT_PREFIXES: &[&str] = &[
        "and ", "al dente", "but ", "from ", "if ", "is fine", "or ", "to ", "until ", "with ",
    ];
    ALLOWED_CONTEXT_PREFIXES
        .iter()
        .any(|prefix| tail.starts_with(prefix))
}

pub(super) fn is_allowed_trailing_prep_context(tail: &str) -> bool {
    let tail = tail.trim_start();
    if tail.is_empty() {
        return true;
    }
    if tail.starts_with(|c: char| !c.is_ascii_alphanumeric()) {
        return true;
    }

    const ALLOWED_CONTEXT_PREFIXES: &[&str] = &[
        "and ",
        "but ",
        "clean",
        "coarse",
        "coarsely ",
        "fine",
        "finely ",
        "for ",
        "freshly ",
        "in ",
        "into ",
        "lightly ",
        "not ",
        "of ",
        "on ",
        "or ",
        "over ",
        "roughly ",
        "small",
        "then ",
        "thin",
        "thinly ",
        "to ",
        "until ",
        "very ",
        "well ",
        "with ",
    ];

    ALLOWED_CONTEXT_PREFIXES
        .iter()
        .any(|prefix| tail.starts_with(prefix))
}

pub(super) fn is_trailing_guidance_note(s: &str) -> bool {
    let normalized = s
        .trim()
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && !c.is_ascii_whitespace())
        .to_lowercase();

    if normalized.is_empty() {
        return false;
    }

    const EXACT_GUIDANCE_NOTES: &[&str] = &[
        "as needed",
        "for garnish",
        "for serving",
        "if desired",
        "more as needed",
        "more or less",
        "or less",
        "or more",
        "or to taste",
        "to taste",
    ];
    if EXACT_GUIDANCE_NOTES.contains(&normalized.as_str()) {
        return true;
    }

    const GUIDANCE_PREFIXES: &[&str] = &[
        "and more for ",
        "and more to taste",
        "approximately ",
        "from approximately ",
        "if ",
        "more for ",
        "more as needed",
        "more to taste",
        "preferably ",
        "or more for ",
        "or more to taste",
        "or substitute ",
        "plus additional to taste",
        "plus extra for ",
        "plus more for ",
        "plus more to taste",
    ];
    if GUIDANCE_PREFIXES
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
    {
        return true;
    }

    (normalized.starts_with("or ")
        && (normalized.contains(" for serving")
            || normalized.contains(" for garnish")
            || normalized.chars().any(|c| c.is_ascii_digit())
            || normalized.contains(" to taste")
            || normalized.contains(" if desired")
            || normalized.contains(" as needed")))
        || normalized.starts_with("or less ")
}

/// Check if a string consists only of prep words (comma-separated).
/// e.g., "finely chopped" -> true, "cooked chicken" -> false, "sliced" -> true
pub(super) fn is_only_prep_words(s: &str) -> bool {
    let s_lower = s.to_lowercase();

    // Split by commas and check each part
    for part in s_lower.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        // Check if this part is entirely prep words
        let words: Vec<&str> = part.split_whitespace().collect();
        if words.is_empty() {
            continue;
        }

        let all_prep = words.iter().all(|word| {
            PREP_NOTES.iter().any(|note| {
                // Check if word matches note exactly or note starts with word
                *word == *note || note.starts_with(&format!("{} ", word))
            })
        });

        if !all_prep {
            return false;
        }
    }

    true
}

/// Lines that should be completely ignored (scraper artifacts, not ingredients or headers).
/// These are checked case-insensitively.
pub(super) const IGNORED_LINE_PATTERNS: &[&str] = &[
    "gather your ingredients",
    "gather the ingredients",
    "here's what you'll need",
    "here's what you need",
    "what you'll need",
    "what you need",
    "you will need",
    "you'll need",
    "ingredients list",
];

/// Bare list/section labels that leak through `detect_section_header` when the
/// source omits a trailing colon. We only filter the mixed/lowercase form;
/// all-caps versions like "SERVE WITH" are intentionally left for
/// `detect_section_header` to consume as section transitions.
pub(super) const STANDALONE_LIST_LABELS: &[&str] = &["ingredients", "serve with"];

pub(super) fn is_standalone_list_label_line(trimmed: &str) -> bool {
    if trimmed.chars().any(|c| c.is_alphabetic())
        && trimmed
            .chars()
            .filter(|c| c.is_alphabetic())
            .all(|c| c.is_uppercase())
    {
        return false;
    }
    let lower = trimmed.to_lowercase();
    STANDALONE_LIST_LABELS.iter().any(|&p| lower == p)
}

/// Prefixes that indicate a line should be ignored (not an ingredient).
pub(super) const IGNORED_LINE_PREFIXES: &[&str] = &[
    "special equipment:",
    "equipment:",
    "tools:",
    "notes:",
    "note:",
    "tip:",
    "tips:",
];

/// Equipment phrases that indicate a line is about kitchen tools, not ingredients.
/// Only compound phrases to avoid false positives (e.g., bare "pan" would match "pan spray").
pub(super) const EQUIPMENT_PHRASES: &[&str] = &[
    "bundt pan",
    "cake pan",
    "tart pan",
    "roasting pan",
    "baking pan",
    "loaf pan",
    "sheet pan",
    "baking sheet",
    "cookie sheet",
    "muffin tin",
    "dutch oven",
    "slow cooker",
    "stand mixer",
    "hand mixer",
    "immersion blender",
    "food processor",
    "pastry bag",
    "piping bag",
    "pastry tip",
    "piping tip",
    "star tip",
    "saucepan",
    "stockpot",
    "skillet",
    "electric mixer",
];

/// Words that indicate a line is about food/ingredients even if it mentions equipment.
pub(super) const INGREDIENT_INDICATOR_WORDS: &[&str] = &[
    "oil",
    "butter",
    "spray",
    "drippings",
    "grease",
    "greasing",
    "paper",
    "parchment",
    "wrap",
    "flour",
    "sugar",
    "salt",
    "water",
    "cream",
    "milk",
    "egg",
    "bread",
    "crumb",
    "seed",
    "nut",
    "walnut",
    "pistachio",
    "pecan",
    "crab",
    "meat",
    "pork",
    "beef",
    "chicken",
    "turkey",
    "cheese",
    "onion",
    "garlic",
    "pepper",
    "tomato",
    "carrot",
    "coconut",
    "lard",
    "frosting",
    "batter",
    "dough",
    "beet",
];

/// Check if a word appears at a word boundary in the text.
/// The word must start at a word boundary (beginning of text or preceded by non-alpha char),
/// but may be followed by additional letters (to allow plurals like "seeds" matching "seed").
pub(super) fn contains_word_prefix(text: &str, word: &str) -> bool {
    for (i, _) in text.match_indices(word) {
        let before_ok = i == 0 || !text.as_bytes()[i - 1].is_ascii_alphabetic();
        if before_ok {
            return true;
        }
    }
    false
}

/// Check if a line describes kitchen equipment rather than an ingredient.
pub(super) fn is_equipment_line(lower: &str) -> bool {
    // Equipment phrases can match as substrings (they're multi-word and specific enough)
    if !EQUIPMENT_PHRASES.iter().any(|&eq| lower.contains(eq)) {
        return false;
    }
    // Ingredient indicators must match as whole words to avoid false matches
    // (e.g., "foil" should not match "oil")
    if INGREDIENT_INDICATOR_WORDS
        .iter()
        .any(|&ing| contains_word_prefix(lower, ing))
    {
        return false;
    }
    true
}

pub(super) fn is_standalone_yield_metadata_line(raw: &str) -> bool {
    let trimmed = raw.trim();
    let unwrapped = if trimmed.starts_with('(') && trimmed.ends_with(')') {
        let closing_idx = trimmed.len() - 1;
        if find_matching_closing_paren(trimmed, 0) == Some(closing_idx) {
            trimmed.get(1..closing_idx).unwrap_or(trimmed).trim()
        } else {
            trimmed
        }
    } else {
        trimmed
    };
    let normalized = unwrapped.trim().trim_end_matches(['.', '!', ';']).trim();
    let lower = normalized.to_lowercase();

    if !lower.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }

    const PREFIXES: &[&str] = &[
        "yield ",
        "yield:",
        "yields ",
        "yields:",
        "serves ",
        "serves:",
        "makes ",
        "makes:",
        "this makes ",
        "this recipe makes ",
    ];

    PREFIXES.iter().any(|prefix| lower.starts_with(prefix))
}

/// Check if a line should be completely ignored (scraper artifact or equipment).
/// Returns true if the line should be skipped entirely.
pub fn should_ignore_line(raw: &str) -> bool {
    let trimmed = raw.trim();
    let lower = trimmed.to_lowercase();

    // Skip lines that are only asterisks (footnote section dividers, e.g., "**")
    if !trimmed.is_empty() && trimmed.chars().all(|c| c == '*') {
        return true;
    }

    // Lines starting with asterisk(s) attached directly to text are footnotes,
    // not ingredients (e.g., "*Substitute octopus with sausage"). Bullet markers
    // like "* 1 cup flour" have whitespace after the asterisk and are stripped
    // by strip_leading_list_marker instead.
    let asterisk_count = trimmed.chars().take_while(|&c| c == '*').count();
    if asterisk_count > 0
        && trimmed
            .chars()
            .nth(asterisk_count)
            .is_some_and(|c| c.is_alphabetic())
    {
        return true;
    }

    if is_standalone_yield_metadata_line(trimmed) {
        return true;
    }

    if is_standalone_list_label_line(trimmed) {
        return true;
    }

    // Check exact matches (case-insensitive)
    for &pattern in IGNORED_LINE_PATTERNS {
        if lower == pattern {
            return true;
        }
    }

    // Check prefixes (case-insensitive)
    for &prefix in IGNORED_LINE_PREFIXES {
        if lower.starts_with(prefix) {
            return true;
        }
    }

    // Check if the line is purely about equipment/tools
    if is_equipment_line(&lower) {
        return true;
    }

    false
}

/// Normalize section header capitalization.
/// - All-caps like "FILLING" → "Filling"
/// - Mixed case like "For the Steak Fajita Marinade" → kept as-is
/// - Lowercase "for the sauce" → "For the Sauce"
pub(super) fn normalize_section_name(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        return name.to_string();
    }

    // Always apply title case for consistent normalization.
    // This handles all-caps headers, lowercase headers, and mixed-case headers
    // like "For the sauce" -> "For the Sauce"
    title_case(name)
}

/// Convert a string to title case.
/// Capitalizes first letter of each word, lowercases the rest.
/// Special handling for small words (the, and, or, of, for, a, an, to, in) - kept lowercase except at start.
pub(super) fn title_case(s: &str) -> String {
    const SMALL_WORDS: &[&str] = &[
        "the", "and", "or", "of", "for", "a", "an", "to", "in", "with",
    ];

    let mut result = String::with_capacity(s.len());
    let mut is_first_word = true;
    let mut current_word = String::new();

    for c in s.chars() {
        if c.is_whitespace() || c == ',' || c == '(' || c == ')' {
            // End of word - flush current word
            if !current_word.is_empty() {
                let word_lower = current_word.to_lowercase();
                if !is_first_word && SMALL_WORDS.contains(&word_lower.as_str()) {
                    result.push_str(&word_lower);
                } else {
                    // Capitalize first letter, lowercase rest
                    let mut chars = current_word.chars();
                    if let Some(first) = chars.next() {
                        result.extend(first.to_uppercase());
                        for ch in chars {
                            result.extend(ch.to_lowercase());
                        }
                    }
                }
                current_word.clear();
                is_first_word = false;
            }
            result.push(c);
        } else {
            current_word.push(c);
        }
    }

    // Flush final word
    if !current_word.is_empty() {
        let word_lower = current_word.to_lowercase();
        if !is_first_word && SMALL_WORDS.contains(&word_lower.as_str()) {
            result.push_str(&word_lower);
        } else {
            let mut chars = current_word.chars();
            if let Some(first) = chars.next() {
                result.extend(first.to_uppercase());
                for ch in chars {
                    result.extend(ch.to_lowercase());
                }
            }
        }
    }

    result
}

/// Detect if a line is a section header (e.g., "For the sauce:", "FILLING:", "Topping Ingredients:").
/// Also detects ALL CAPS lines without colons (e.g., "DOUGH", "FILLING", "BERRY SAUCE").
/// Returns Some(normalized_section_name) if it's a header, None if it's a regular ingredient.
/// Section names are normalized: "FILLING" → "Filling", "for the sauce" → "For the Sauce".
pub fn detect_section_header(raw: &str) -> Option<String> {
    let trimmed = raw.trim();

    // Check for ALL CAPS without colon first (e.g., "DOUGH", "FILLING", "BERRY SAUCE")
    // Pattern: Line is entirely uppercase letters/spaces, no digits, reasonable length
    if trimmed.len() <= 40
        && !trimmed.contains(':')
        && !trimmed.chars().any(|c| c.is_ascii_digit())
        && trimmed
            .chars()
            .filter(|c| c.is_alphabetic())
            .all(|c| c.is_uppercase())
        && trimmed.chars().any(|c| c.is_alphabetic())
    {
        return Some(normalize_section_name(trimmed));
    }

    // Must end with colon - strip it to get the section name
    let name = trimmed.strip_suffix(':')?.trim();

    // Must not be empty
    if name.is_empty() {
        return None;
    }

    // Try parsing it - if we get an amount, it's likely an ingredient, not a header
    let parsed = parse_ingredient(raw);
    if !parsed.measurements.is_empty()
        && parsed.measurements[0].amount.is_some()
        && parsed.measurements[0].unit.is_some()
    {
        // Has both amount and unit - probably an ingredient
        return None;
    }

    // Check if it matches common header patterns
    let name_lower = name.to_lowercase();

    // Pattern 1: Ends with "Ingredients" (e.g., "Topping Ingredients", "Crust Ingredients")
    if name_lower.ends_with("ingredients") || name_lower.ends_with("ingredient") {
        return Some(normalize_section_name(name));
    }

    // Pattern 2: "For the X" or "For X" patterns
    if name_lower.starts_with("for the ") || name_lower.starts_with("for ") {
        return Some(normalize_section_name(name));
    }

    // Pattern 2b: Imperative "To X" / "To the X" patterns
    // ("To assemble:", "To serve:", "To finish:", "To assemble the cake:")
    if name_lower.starts_with("to ") {
        return Some(normalize_section_name(name));
    }

    // Pattern 3: All-caps short names (FILLING, DRIZZLE, TOPPING, SAUCE, etc.)
    // Must be reasonably short and mostly uppercase letters/spaces
    if name.len() <= 40
        && name
            .chars()
            .filter(|c| c.is_alphabetic())
            .all(|c| c.is_uppercase())
        && name.chars().any(|c| c.is_alphabetic())
    {
        return Some(normalize_section_name(name));
    }

    // Pattern 4: Short colon-terminated headers with no digits.
    // We already rejected anything that parses with both amount + unit above,
    // so by here the line has no obvious ingredient shape. Treat short,
    // digit-free, few-word phrases as section headers (e.g. "Dough:",
    // "Asparagus pesto:", "Chicken and noodle salad:").
    let word_count = name.split_whitespace().count();
    if name.len() <= 50 && word_count <= 5 && !name.chars().any(|c| c.is_ascii_digit()) {
        return Some(normalize_section_name(name));
    }

    // Pattern 5: Longer mixed-case headers containing a well-known section
    // keyword. Short phrases are already covered by pattern 4; this fallback
    // catches verbose labels like "Creamy Artichoke Spread (makes a little
    // extra):" or "PART III: The ham and nut filling:" where the word count
    // exceeds five but a section keyword gives us high confidence.
    if name.len() <= 80 {
        const SECTION_KEYWORDS: &[&str] = &[
            "topping", "filling", "frosting", "icing", "glaze", "sauce", "marinade", "dressing",
            "crust", "batter", "drizzle", "garnish", "assembly", "serving", "optional", "coating",
            "base", "cream", "streusel", "crumble", "spread",
        ];
        if SECTION_KEYWORDS.iter().any(|kw| name_lower.contains(kw)) {
            return Some(normalize_section_name(name));
        }
    }

    None
}

/// Split a compound item string into individual items.
///
/// Splitting rules:
/// 1. Replace ", and " with ", " (normalize Oxford comma)
/// 2. Split on ", "
/// 3. Further split each part on " and "
/// 4. Trim each result, drop empties
pub(super) fn split_compound_items(item: &str) -> Vec<String> {
    // Normalize Oxford commas
    let normalized = item.replace(", and ", ", ");

    let mut result = Vec::new();
    for part in normalized.split(", ") {
        for sub in part.split(" and ") {
            let trimmed = sub.trim();
            if !trimmed.is_empty() {
                result.push(trimmed.to_string());
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_section_header_for_the_pattern() {
        // "For the X" patterns should be normalized
        assert_eq!(
            detect_section_header("For the sauce:"),
            Some("For the Sauce".to_string())
        );
        assert_eq!(
            detect_section_header("For the dough:"),
            Some("For the Dough".to_string())
        );
        assert_eq!(
            detect_section_header("For serving:"),
            Some("For Serving".to_string())
        );
        // Already title case should be preserved
        assert_eq!(
            detect_section_header("For the Steak Fajita Marinade:"),
            Some("For the Steak Fajita Marinade".to_string())
        );
    }

    #[test]
    fn test_detect_section_header_all_caps() {
        // All-caps headers WITH colon should be normalized to title case
        assert_eq!(
            detect_section_header("FILLING:"),
            Some("Filling".to_string())
        );
        assert_eq!(
            detect_section_header("DRIZZLE:"),
            Some("Drizzle".to_string())
        );
        assert_eq!(
            detect_section_header("TOPPING:"),
            Some("Topping".to_string())
        );
        assert_eq!(
            detect_section_header("TOPPINGS, OPTIONAL:"),
            Some("Toppings, Optional".to_string())
        );
        assert_eq!(
            detect_section_header("FOR THE SAUCE:"),
            Some("For the Sauce".to_string())
        );
    }

    #[test]
    fn test_detect_section_header_all_caps_no_colon() {
        // All-caps headers WITHOUT colon should also be detected
        assert_eq!(
            detect_section_header("FILLING"),
            Some("Filling".to_string())
        );
        assert_eq!(detect_section_header("DOUGH"), Some("Dough".to_string()));
        assert_eq!(
            detect_section_header("ASSEMBLY"),
            Some("Assembly".to_string())
        );
        assert_eq!(
            detect_section_header("BERRY SAUCE"),
            Some("Berry Sauce".to_string())
        );
        assert_eq!(
            detect_section_header("WHIPPED COTTAGE CHEESE"),
            Some("Whipped Cottage Cheese".to_string())
        );
        assert_eq!(
            detect_section_header("TOASTS AND ASSEMBLY"),
            Some("Toasts and Assembly".to_string())
        );
    }

    #[test]
    fn test_detect_section_header_ingredients_suffix() {
        // Lines ending with "Ingredients" should be detected
        assert_eq!(
            detect_section_header("Topping Ingredients:"),
            Some("Topping Ingredients".to_string())
        );
        assert_eq!(
            detect_section_header("Crust Ingredients:"),
            Some("Crust Ingredients".to_string())
        );
        assert_eq!(
            detect_section_header("Optional Frosting Ingredients:"),
            Some("Optional Frosting Ingredients".to_string())
        );
    }

    #[test]
    fn test_detect_section_header_mixed_case_keywords() {
        // Mixed-case headers with section keywords should be detected
        assert_eq!(
            detect_section_header("Toppings:"),
            Some("Toppings".to_string())
        );
        assert_eq!(
            detect_section_header("optional toppings:"),
            Some("Optional Toppings".to_string())
        );
        assert_eq!(
            detect_section_header("Cream cheese filling:"),
            Some("Cream Cheese Filling".to_string())
        );
        assert_eq!(
            detect_section_header("Chocolate Glaze:"),
            Some("Chocolate Glaze".to_string())
        );
        assert_eq!(
            detect_section_header("For serving:"),
            Some("For Serving".to_string())
        );
    }

    #[test]
    fn test_detect_section_header_long_keyword_fallback() {
        // Headers longer than five words still count as sections when they
        // contain a well-known section keyword.
        assert!(detect_section_header("Creamy Artichoke Spread (makes a little extra):").is_some());
        assert!(detect_section_header("PART III: The ham and nut filling:").is_some());
        // A long phrase without any recognized keyword stays an ingredient
        // (we don't have enough signal to call it a header).
        assert_eq!(
            detect_section_header("This is a long phrase with no keyword at all:"),
            None
        );
    }

    #[test]
    fn test_detect_section_header_to_assemble_pattern() {
        // Imperative "To X:" headers (parallel to "For X:") should be detected
        assert_eq!(
            detect_section_header("To assemble:"),
            Some("To Assemble".to_string())
        );
        assert_eq!(
            detect_section_header("To serve:"),
            Some("To Serve".to_string())
        );
        assert_eq!(
            detect_section_header("To finish:"),
            Some("To Finish".to_string())
        );
        assert_eq!(
            detect_section_header("To assemble the cake:"),
            Some("To Assemble the Cake".to_string())
        );
    }

    #[test]
    fn test_detect_section_header_single_word() {
        // Single-word headers ending with colon should be detected
        assert_eq!(detect_section_header("Dough:"), Some("Dough".to_string()));
        assert_eq!(detect_section_header("Brine:"), Some("Brine".to_string()));
        assert_eq!(
            detect_section_header("Chicken:"),
            Some("Chicken".to_string())
        );
        assert_eq!(detect_section_header("Eggs:"), Some("Eggs".to_string()));
        assert_eq!(
            detect_section_header("Caramel:"),
            Some("Caramel".to_string())
        );
        assert_eq!(
            detect_section_header("Meatballs:"),
            Some("Meatballs".to_string())
        );
    }

    #[test]
    fn test_detect_section_header_not_header() {
        // Regular ingredients should return None
        assert_eq!(detect_section_header("1 cup flour"), None);
        assert_eq!(detect_section_header("2 tablespoons oil"), None);
        // Ingredient with colon in note should not be detected as header
        assert_eq!(detect_section_header("butter: softened"), None);
    }

    #[test]
    fn test_should_ignore_line() {
        // Scraper artifacts should be ignored
        assert!(should_ignore_line("Gather Your Ingredients"));
        assert!(should_ignore_line("gather your ingredients"));
        assert!(should_ignore_line("GATHER YOUR INGREDIENTS"));
        assert!(should_ignore_line("Special equipment: Spice grinder"));
        assert!(should_ignore_line("Equipment: Stand mixer"));
        assert!(should_ignore_line("Notes: See recipe headnotes"));
        assert!(should_ignore_line("(This makes a 12-ounce batch.)"));
        assert!(should_ignore_line("Yield: 10 3-inch biscuits"));
        assert!(should_ignore_line("Yield 26 to 28 cookies"));
        assert!(should_ignore_line("Yields: 12 bars"));
        assert!(should_ignore_line("Serves 4 generously"));
        assert!(should_ignore_line("Makes 12 muffins"));
        assert!(should_ignore_line("Ingredients"));
        assert!(should_ignore_line("ingredients"));
        assert!(should_ignore_line("  ingredients  "));
        assert!(should_ignore_line("Serve with"));
        assert!(should_ignore_line("serve with"));
        // All-caps versions are intentionally NOT filtered so that
        // detect_section_header can promote them to section transitions.
        assert!(!should_ignore_line("INGREDIENTS"));
        assert!(!should_ignore_line("SERVE WITH"));

        // Regular ingredients should not be ignored
        assert!(!should_ignore_line("1 cup flour"));
        assert!(!should_ignore_line("salt to taste"));
        assert!(!should_ignore_line("For the sauce:"));
        assert!(!should_ignore_line(
            "40 grams raspberries (about 10 raspberries)"
        ));
        assert!(!should_ignore_line("(about 10 raspberries)"));
        // Longer phrases containing the same words must still pass through.
        assert!(!should_ignore_line("Cake Ingredients:"));
        assert!(!should_ignore_line("Serve with crackers"));
    }

    #[test]
    fn test_should_ignore_equipment_lines() {
        // Equipment-only lines should be ignored
        assert!(should_ignore_line(
            "A 12-cup Bundt pan, a pastry bag, and a large star tip"
        ));
        assert!(should_ignore_line(
            "A 9½\"-diameter tart pan with removable bottom"
        ));
        assert!(should_ignore_line(
            "Large dutch oven, pot, or high-sided skillet."
        ));
        assert!(should_ignore_line(
            "A dutch oven or large heavy-bottomed pot"
        ));
        assert!(should_ignore_line(
            "9  \" wide cake pan with 2\" tall sides."
        ));
        assert!(should_ignore_line(
            "2 9x13 metal baking pans or (1) large roasting pan lined with foil."
        ));
        assert!(should_ignore_line("11.38 x 16.5 lipped baking pan"));
        assert!(should_ignore_line("6 QT Slow Cooker"));
        assert!(should_ignore_line(
            "Hand immersion blender (preferred & easier, or food processor, or blender)"
        ));
        assert!(should_ignore_line("stand mixer"));
        assert!(should_ignore_line("food processor"));
        assert!(should_ignore_line(
            "1 large disposable aluminum roasting pan (if using charcoal)"
        ));

        // Ingredient lines mentioning equipment should NOT be ignored
        assert!(!should_ignore_line("Pan drippings from roasting pan"));
        assert!(!should_ignore_line("oil, for the pan"));
        assert!(!should_ignore_line("butter for the pan"));
        assert!(!should_ignore_line("pan spray"));
        assert!(!should_ignore_line("Parchment paper"));
        assert!(!should_ignore_line(
            "4 18×13-inch (approximately) pieces parchment paper"
        ));
        assert!(!should_ignore_line(
            "8  16-inch long pieces of Reynolds Wrap Heavy Duty Foil"
        ));
        assert!(!should_ignore_line("Vegetable oil, for greasing pan"));
        assert!(!should_ignore_line("2 cups pan drippings liquid"));
        assert!(!should_ignore_line(
            "ghee (butter or coconut oil (to coat skillet))"
        ));
        assert!(!should_ignore_line(
            "2 cups panko breadcrumbs, pulsed in a food processor"
        ));
        assert!(!should_ignore_line(
            "1 tsp caraway seeds, (toasted and ground (use food processor))"
        ));
    }

    #[test]
    fn test_title_case() {
        assert_eq!(title_case("FILLING"), "Filling");
        assert_eq!(title_case("FOR THE SAUCE"), "For the Sauce");
        assert_eq!(title_case("TOPPINGS, OPTIONAL"), "Toppings, Optional");
        assert_eq!(title_case("for the dough"), "For the Dough");
        assert_eq!(
            title_case("FOR THE STEAK FAJITA MARINADE"),
            "For the Steak Fajita Marinade"
        );
    }

    #[test]
    fn test_is_only_prep_words() {
        // Single prep words
        assert!(is_only_prep_words("sliced"));
        assert!(is_only_prep_words("chopped"));
        assert!(is_only_prep_words("cooked"));
        assert!(is_only_prep_words("toasted"));
        assert!(is_only_prep_words("roasted"));

        // Multiple prep words
        assert!(is_only_prep_words("finely chopped"));
        assert!(is_only_prep_words("thinly sliced"));

        // Not only prep words (contains actual ingredient)
        assert!(!is_only_prep_words("cooked chicken"));
        assert!(!is_only_prep_words("chicken"));
        assert!(!is_only_prep_words("sliced onions"));

        // Edge cases
        assert!(is_only_prep_words("")); // empty returns true (no non-prep words)
        assert!(!is_only_prep_words("salt"));
        assert!(!is_only_prep_words("butter"));
    }

    #[test]
    fn test_trailing_prep_note_requires_prep_phrase_not_noun_phrase() {
        assert!(is_trailing_prep_note("finely chopped"));
        assert!(is_trailing_prep_note("peeled and chopped"));
        assert!(is_trailing_prep_note("chopped very small"));
        assert!(is_trailing_prep_note("scrubbed clean"));
        assert!(is_trailing_prep_note("scrubbed clean but not peeled"));
        assert!(is_trailing_prep_note("thinly sliced for garnish"));
        assert!(is_trailing_prep_note("cooled to room temperature"));
        assert!(!is_trailing_prep_note("unseasoned dried breadcrumbs"));
        assert!(!is_trailing_prep_note("cooked chicken meat"));
    }

    #[test]
    fn test_trailing_guidance_note_allows_non_prep_suffixes() {
        assert!(is_trailing_guidance_note("or to taste"));
        assert!(is_trailing_guidance_note("or more to taste"));
        assert!(is_trailing_guidance_note("more for serving"));
        assert!(is_trailing_guidance_note("plus extra for seasoning"));
        assert!(!is_trailing_guidance_note("unseasoned dried breadcrumbs"));
    }

    #[test]
    fn test_orphaned_prep_word_fallback() {
        // When the item would be just a prep word, return raw
        let result = parse_ingredient("sliced");
        assert_eq!(result.item, "sliced");

        // When comma is between prep words and ingredient, don't split there
        // "2 cups finely chopped, cooked chicken meat" should NOT extract
        // "cooked chicken meat" as note, leaving "finely chopped" as item
        let result = parse_ingredient("2 cups finely chopped, cooked chicken meat");
        // Item should include the ingredient, not just prep words
        assert_eq!(result.item, "finely chopped, cooked chicken meat");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_trailing_scrubbed_clean_is_note_not_item_identity() {
        let result = parse_ingredient("4 russet potatoes, scrubbed clean");
        assert_eq!(result.item, "russet potatoes");
        assert_eq!(result.note, Some("scrubbed clean".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("4".to_string()));
        assert_eq!(result.measurements[0].unit, None);
    }

    #[test]
    fn test_trailing_guidance_suffix_is_note_not_item_identity() {
        let result = parse_ingredient("0.25 cup pasta sauce, or to taste");
        assert_eq!(result.item, "pasta sauce");
        assert_eq!(result.note, Some("or to taste".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("0.25".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_mixed_trailing_prep_guidance_suffix_is_note_not_item_identity() {
        let result = parse_ingredient("Oranges, thinly sliced for garnish");
        assert_eq!(result.item, "Oranges");
        assert_eq!(result.note, Some("thinly sliced for garnish".to_string()));
        assert!(result.measurements.is_empty());
    }

    #[test]
    fn test_split_compound_items() {
        assert_eq!(
            split_compound_items("salt and pepper"),
            vec!["salt", "pepper"]
        );
        assert_eq!(
            split_compound_items("dried oregano and cumin"),
            vec!["dried oregano", "cumin"]
        );
        assert_eq!(
            split_compound_items("ground cinnamon, ginger, cloves, and cardamom"),
            vec!["ground cinnamon", "ginger", "cloves", "cardamom"]
        );
        assert_eq!(
            split_compound_items("chili powder, onion powder, and garlic powder"),
            vec!["chili powder", "onion powder", "garlic powder"]
        );
        assert_eq!(
            split_compound_items("garlic and onion powder"),
            vec!["garlic", "onion powder"]
        );
        // Single item - no split
        assert_eq!(
            split_compound_items("pork tenderloins"),
            vec!["pork tenderloins"]
        );
    }

    #[test]
    fn test_should_ignore_standalone_asterisks() {
        assert!(should_ignore_line("**"));
        assert!(should_ignore_line("*"));
        assert!(should_ignore_line("***"));
        // Regular ingredients should not be ignored
        assert!(!should_ignore_line("1 cup flour*"));
    }
}
