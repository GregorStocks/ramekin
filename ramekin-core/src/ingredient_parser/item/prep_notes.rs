/// Common preparation notes
pub(in crate::ingredient_parser) const PREP_NOTES: &[&str] = &[
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
/// Check if a string looks like a preparation note.
pub(in crate::ingredient_parser) fn is_prep_note(s: &str) -> bool {
    let s_lower = s.to_lowercase();
    PREP_NOTES.iter().any(|note| s_lower.contains(note))
}

pub(in crate::ingredient_parser) fn is_trailing_prep_note(s: &str) -> bool {
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

pub(in crate::ingredient_parser) fn is_strict_trailing_prep_note(s: &str) -> bool {
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

pub(in crate::ingredient_parser) fn is_trailing_prep_note_with_context(s: &str) -> bool {
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

pub(in crate::ingredient_parser) const ACTIVE_PREP_PREFIXES: &[&str] = &[
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

pub(in crate::ingredient_parser) fn contains_active_prep_note(s: &str) -> bool {
    let normalized = s.to_lowercase();
    ACTIVE_PREP_PREFIXES.iter().any(|prefix| {
        normalized
            .match_indices(prefix)
            .any(|(idx, _)| has_word_boundaries(&normalized, idx, prefix.len()))
    })
}

pub(in crate::ingredient_parser) fn has_word_boundaries(s: &str, start: usize, len: usize) -> bool {
    let before = s.get(..start).and_then(|prefix| prefix.chars().next_back());
    let after = s
        .get(start + len..)
        .and_then(|suffix| suffix.chars().next());
    before.is_none_or(|c| !c.is_ascii_alphabetic())
        && after.is_none_or(|c| !c.is_ascii_alphabetic())
}

pub(in crate::ingredient_parser) fn ambiguous_prep_note_has_allowed_context(s: &str) -> bool {
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

pub(in crate::ingredient_parser) fn is_allowed_ambiguous_prep_context(tail: &str) -> bool {
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

pub(in crate::ingredient_parser) fn is_allowed_trailing_prep_context(tail: &str) -> bool {
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

pub(in crate::ingredient_parser) fn is_trailing_guidance_note(s: &str) -> bool {
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
pub(in crate::ingredient_parser) fn is_only_prep_words(s: &str) -> bool {
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
