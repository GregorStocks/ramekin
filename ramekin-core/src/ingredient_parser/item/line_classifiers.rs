use crate::ingredient_parser::{find_matching_closing_paren, parse_ingredient};

/// Lines that should be completely ignored (scraper artifacts, not ingredients or headers).
/// These are checked case-insensitively.
pub(in crate::ingredient_parser) const IGNORED_LINE_PATTERNS: &[&str] = &[
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
pub(in crate::ingredient_parser) const STANDALONE_LIST_LABELS: &[&str] =
    &["ingredients", "serve with"];

pub(in crate::ingredient_parser) fn is_standalone_list_label_line(trimmed: &str) -> bool {
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
pub(in crate::ingredient_parser) const IGNORED_LINE_PREFIXES: &[&str] = &[
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
pub(in crate::ingredient_parser) const EQUIPMENT_PHRASES: &[&str] = &[
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
pub(in crate::ingredient_parser) const INGREDIENT_INDICATOR_WORDS: &[&str] = &[
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
pub(in crate::ingredient_parser) fn contains_word_prefix(text: &str, word: &str) -> bool {
    for (i, _) in text.match_indices(word) {
        let before_ok = i == 0 || !text.as_bytes()[i - 1].is_ascii_alphabetic();
        if before_ok {
            return true;
        }
    }
    false
}

/// Check if a line describes kitchen equipment rather than an ingredient.
pub(in crate::ingredient_parser) fn is_equipment_line(lower: &str) -> bool {
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

pub(in crate::ingredient_parser) fn is_standalone_yield_metadata_line(raw: &str) -> bool {
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
pub(in crate::ingredient_parser) fn normalize_section_name(name: &str) -> String {
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
pub(in crate::ingredient_parser) fn title_case(s: &str) -> String {
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

pub(in crate::ingredient_parser) fn is_known_title_case_section_label(name: &str) -> bool {
    const TITLE_CASE_SECTION_LABELS: &[&str] = &[
        "assembly", "batter", "coating", "crumb", "crumble", "crust", "dough", "dressing",
        "filling", "frosting", "garnish", "glaze", "icing", "lid", "marinade", "puffs", "sauce",
        "streusel", "syrup", "topping",
    ];
    const TITLE_CASE_SECTION_PHRASES: &[&str] = &["brown sugar coating", "cream cheese glaze"];

    if name.contains(':')
        || name.chars().any(|c| c.is_ascii_digit())
        || !name.chars().any(|c| c.is_alphabetic())
    {
        return false;
    }

    let words = name.split_whitespace();
    if !words
        .clone()
        .all(|word| word.chars().next().is_some_and(|c| c.is_uppercase()))
    {
        return false;
    }

    let word_count = words.clone().count();
    let lower = name.to_lowercase();
    if word_count == 1 {
        return TITLE_CASE_SECTION_LABELS
            .iter()
            .any(|label| lower == *label);
    }

    TITLE_CASE_SECTION_PHRASES
        .iter()
        .any(|phrase| lower == *phrase)
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

    if is_known_title_case_section_label(trimmed) {
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
