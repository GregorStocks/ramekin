//! Line filtering, prep/guidance notes, section headers, and item splitting.

mod line_classifiers;
mod normalize;
mod prep_notes;

pub(in crate::ingredient_parser) use line_classifiers::normalize_section_name;
pub use line_classifiers::{detect_section_header, should_ignore_line};
pub(in crate::ingredient_parser) use normalize::*;
pub(in crate::ingredient_parser) use prep_notes::*;

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

/// Prepositions that, following a leading prep word, mark a comma part like
/// "packed in oil" or "cut into strips" as a preparation note rather than a
/// standalone ingredient ("ground cinnamon" has a noun there instead).
const PREP_PHRASE_PREPOSITIONS: &[&str] = &[
    "and", "before", "dry", "for", "in", "into", "of", "on", "over", "then", "to", "until", "with",
];

/// Comma-part prefixes that mark a qualifier for the preceding item rather
/// than a separate ingredient (e.g. "salt, for the pot", "olive oil, plus more").
const NON_ITEM_PART_PREFIXES: &[&str] = &[
    "about ",
    "additional ",
    "any ",
    "approximately ",
    "as ",
    "at ",
    "extra ",
    "for ",
    "if ",
    "in ",
    "into ",
    "like ",
    "more ",
    "or ",
    "other ",
    "plus ",
    "preferably ",
    "such as ",
    "to ",
    "with ",
    "without ",
];

/// Adjectives that coordinate over a shared final noun ("red, yellow, and
/// green bell peppers"); as a single-word comma part they modify the last
/// part's item rather than naming an ingredient of their own.
const MODIFIER_ONLY_WORDS: &[&str] = &[
    "big", "black", "blue", "bright", "brown", "dark", "deep", "green", "hot", "large", "light",
    "little", "medium", "mild", "orange", "pale", "pink", "purple", "red", "small", "spicy",
    "sweet", "white", "yellow",
];

/// Nouns that name a component of the preceding item ("lemon, zest and
/// juice", "eggs, whites and yolks separated") rather than an ingredient of
/// their own.
const COMPONENT_TAIL_WORDS: &[&str] = &[
    "bones", "caps", "flesh", "fronds", "juice", "peel", "pulp", "rind", "seeds", "shells", "skin",
    "stalks", "stems", "tops", "whites", "yolks", "zest",
];

/// True if a comma part reads as a qualifier of the preceding item (a prep,
/// guidance, or other trailing note) rather than a standalone ingredient.
fn is_non_item_part(part: &str) -> bool {
    let lower = part.to_lowercase();
    // Match both "more ..." qualifiers and a bare trailing word ("and more").
    if NON_ITEM_PART_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix) || lower == prefix.trim_end())
    {
        return true;
    }
    let lower_word_list: Vec<&str> = lower.split_whitespace().collect();
    if !lower_word_list.is_empty()
        && lower_word_list
            .iter()
            .all(|word| MODIFIER_ONLY_WORDS.contains(word))
    {
        return true;
    }
    if lower_word_list.len() == 1 && COMPONENT_TAIL_WORDS.contains(&lower_word_list[0]) {
        return true;
    }
    if is_only_prep_words(&lower)
        || is_strict_trailing_prep_note(&lower)
        || is_trailing_prep_note_with_context(&lower)
        || is_trailing_guidance_note(&lower)
    {
        return true;
    }

    let words: Vec<&str> = lower.split_whitespace().collect();
    let is_prep_word =
        |word: &str| PREP_NOTES.contains(&word) || ACTIVE_PREP_PREFIXES.contains(&word);
    // "packed in oil": prep word followed by a preposition is a prep phrase.
    if let (Some(first), Some(second)) = (words.first(), words.get(1)) {
        if is_prep_word(first) && PREP_PHRASE_PREPOSITIONS.contains(second) {
            return true;
        }
    }
    // "skin removed": a multi-word part ending in an active prep participle
    // describes the preceding item rather than naming a new one.
    if words.len() >= 2 {
        if let Some(last) = words.last() {
            if ACTIVE_PREP_PREFIXES.contains(last) {
                return true;
            }
        }
    }

    false
}

/// Phrases that mark a comma line as prose or qualified guidance rather than
/// a plain list of items ("sprinkles and maraschino cherries for serving",
/// "grated hard cheese to taste, Parmesan and Pecorino").
const NON_LIST_PHRASES: &[&str] = &[
    "and/or",
    " for ",
    " to ",
    " such as ",
    " see ",
    " below",
    " above",
    " etc",
    " if ",
    " as needed",
    " as desired",
    " optional",
    "recipe follows",
    "however",
    "whatever",
];

/// Split a bare compound ingredient line like "salt, pepper and cumin" into
/// its component items.
///
/// Deliberately conservative; returns `None` unless the whole line reads as
/// "a, b[, c…] and z" — a plain comma list of short, amount-free item names
/// closed by a final "and":
/// - any digit, unicode fraction, or spelled-out amount ("One teaspoon")
///   disqualifies the line (with amounts in play, comma tails are usually
///   notes or measurements, not separate ingredients, and "each" lines have
///   their own expansion)
/// - parentheticals, colons, semicolons, "&", " or ", prose/guidance phrases
///   ("for serving", "see notes", "if desired", "as needed", "and/or"), a
///   leading bare measurement unit or quantity determiner ("pinch each of
///   ...", "a few sprigs of ..."), and "each" all disqualify it
/// - the final "and" (or ", and") is required: without it, two-part lines are
///   as likely "item, qualifier" ("Fresh parsley, garnish", "oil, a drizzle")
///   as two items, and plain "salt and pepper" pairs may name one compound
///   item; an " and " anywhere else isn't list grammar and disqualifies too
/// - every part must read as a standalone item: short, and not a prep,
///   guidance, or other trailing qualifier ("finely chopped", "to taste",
///   "for garnish", "packed in oil")
/// - at most one part after the first may be capitalized, so title-like lines
///   ("Vanilla, Pistachio and Strawberry Mini Loaf Cakes") stay intact
pub(super) fn split_bare_compound_line(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim().trim_end_matches(['.', ',']).trim_end();
    if trimmed.contains(['(', ')', ':', ';', '&']) {
        return None;
    }
    // Spelled-out amounts ("One teaspoon each ...") count as amounts too, so
    // normalize word numbers before checking, matching parse_ingredient.
    let normalized_amounts = super::amounts::normalize_word_numbers(trimmed);
    if normalized_amounts
        .chars()
        .any(|c| c.is_ascii_digit() || super::unicode_fraction_ascii(c).is_some())
    {
        return None;
    }
    if !trimmed.contains(',') {
        return None;
    }
    let lower = trimmed.to_lowercase();
    if lower.contains(" or ") || NON_LIST_PHRASES.iter().any(|phrase| lower.contains(phrase)) {
        return None;
    }

    // Lines led by a bare measurement unit carry an amount even without a
    // number ("pinch each of salt, pepper, and cumin"); leave them to the
    // measurement parser and "each" expansion. "each" anywhere means the
    // expansion owns the line.
    let lower_words: Vec<&str> = lower.split_whitespace().collect();
    if lower_words.contains(&"each") {
        return None;
    }
    let mut unit_scan_start = 0;
    while lower_words.get(unit_scan_start).is_some_and(|word| {
        *word == "a" || *word == "an" || super::units::MEASUREMENT_MODIFIERS.contains(word)
    }) {
        unit_scan_start += 1;
    }
    // Quantity determiners ("a few sprigs of ...", "several ...") carry
    // shared amount context just like units do.
    const QUANTITY_DETERMINERS: &[&str] = &[
        "couple", "few", "many", "numerous", "plenty", "several", "some",
    ];
    if lower_words
        .get(unit_scan_start)
        .is_some_and(|word| QUANTITY_DETERMINERS.contains(word))
    {
        return None;
    }
    let leading_words = &lower_words[unit_scan_start..];
    let leads_with_unit = match (leading_words.first(), leading_words.get(1)) {
        (Some(first), second) => {
            super::units::UNITS_RAW.contains(first)
                || second.is_some_and(|second| {
                    let joined = format!("{} {}", first, second);
                    super::units::UNITS_RAW.iter().any(|unit| *unit == joined)
                })
        }
        (None, _) => false,
    };
    if leads_with_unit {
        return None;
    }

    let comma_parts: Vec<&str> = trimmed.split(',').map(str::trim).collect();
    let mut parts: Vec<String> = Vec::new();
    let mut saw_final_and = false;
    for (idx, part) in comma_parts.iter().enumerate() {
        let is_last = idx == comma_parts.len() - 1;
        // Oxford comma: "a, b, and c" leaves a leading "and" on the last part.
        let part = if is_last {
            if let Some(stripped) = part.strip_prefix("and ") {
                saw_final_and = true;
                stripped.trim()
            } else {
                part
            }
        } else {
            part
        };
        if part.is_empty() {
            return None;
        }
        if let Some((before, after)) = part.split_once(" and ") {
            // "and" anywhere but the final list position isn't list grammar.
            if !is_last || saw_final_and || after.contains(" and ") {
                return None;
            }
            saw_final_and = true;
            parts.push(before.trim().to_string());
            parts.push(after.trim().to_string());
        } else {
            parts.push(part.to_string());
        }
    }

    if !saw_final_and || parts.len() < 2 {
        return None;
    }
    // "garlic, onion and chili powders": a plural head noun shared across the
    // list means the earlier parts modify it rather than name ingredients.
    const SHARED_HEAD_TAIL_WORDS: &[&str] = &[
        "cheeses", "extracts", "flours", "juices", "mustards", "oils", "pastes", "peppers",
        "powders", "purees", "salts", "sauces", "seeds", "sugars", "syrups", "vinegars", "zests",
    ];
    if parts.last().is_some_and(|last| {
        let words: Vec<String> = last
            .split_whitespace()
            .map(|word| word.to_lowercase())
            .collect();
        words.len() >= 2
            && words
                .last()
                .is_some_and(|word| SHARED_HEAD_TAIL_WORDS.contains(&word.as_str()))
    }) {
        return None;
    }
    // Title-like lines capitalize most parts; item lists rarely capitalize
    // anything past the first part.
    let capitalized_tail_parts = parts
        .iter()
        .skip(1)
        .filter(|part| part.chars().next().is_some_and(|c| c.is_uppercase()))
        .count();
    if capitalized_tail_parts >= 2 {
        return None;
    }
    for part in &parts {
        if !part.chars().any(|c| c.is_alphabetic()) {
            return None;
        }
        if part.split_whitespace().count() > 5 {
            return None;
        }
        if is_non_item_part(part) {
            return None;
        }
    }

    Some(parts)
}

#[cfg(test)]
mod tests;
