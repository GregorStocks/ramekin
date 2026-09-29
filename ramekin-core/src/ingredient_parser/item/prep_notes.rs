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

/// Comma parts that qualify the preceding item rather than name it: purposes
/// ("for frying"), examples ("such as canola"), references ("see note"), and
/// amount guidance ("about 8 ounces", "divided").
pub(in crate::ingredient_parser) fn is_trailing_qualifier(s: &str) -> bool {
    const QUALIFIER_PREFIXES: &[&str] = &[
        "about ",
        "approximately ",
        "at least ",
        "each ",
        "for ",
        "from ",
        "if ",
        "in ",
        "including ",
        "like ",
        "or your ",
        "plus ",
        "preferably ",
        "see ",
        "such as ",
        "with ",
        "without ",
        "you'll ",
        "you will ",
    ];
    const QUALIFIER_WORDS: &[&str] = &[
        "divided",
        "homemade or store bought",
        "homemade or store-bought",
        "homemade or storebought",
        "optional",
        "separated",
        "spun dry",
        "store bought or homemade",
        "store-bought or homemade",
        "thawed",
        "to finish",
        "to serve",
        "warmed",
    ];
    // Participles that end a prep phrase ("crusts removed", "stems and seeds
    // discarded"): the part describes the item rather than naming another.
    const TRAILING_PARTICIPLES: &[&str] = &[
        "discarded",
        "removed",
        "reserved",
        "separated",
        "thawed",
        "trimmed",
        "warmed",
    ];
    let lower = s
        .trim()
        .trim_matches(|c: char| c == '[' || c == ']' || c == '.' || c.is_whitespace())
        .to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    // "any flavor", "any percentage will do": an attribute left open, not an
    // alternative ("or frankly, any old dish").
    const OPEN_ATTRIBUTES: &[&str] = &[
        "brand",
        "color",
        "favorite",
        "flavor",
        "kind",
        "percentage",
        "shape",
        "size",
        "style",
        "temperature",
        "thickness",
        "type",
        "variety",
    ];
    let open_attribute = words.first() == Some(&"any")
        && words
            .get(1)
            .is_some_and(|word| OPEN_ATTRIBUTES.contains(&word.trim_end_matches(',')));
    open_attribute
        || QUALIFIER_PREFIXES
            .iter()
            .any(|prefix| lower.starts_with(prefix))
        || QUALIFIER_WORDS.contains(&lower.as_str())
        || (words.len() >= 2
            && words
                .last()
                .is_some_and(|w| TRAILING_PARTICIPLES.contains(w)))
}

/// Whether an item names a category rather than a food ("Garnishes", "taco
/// toppings", "chopped mix-ins"), so examples after it ("such as chives",
/// "like cooked meats") are the actual foods.
pub(in crate::ingredient_parser) fn has_generic_head(item: &str) -> bool {
    const GENERIC_HEADS: &[&str] = &[
        "accompaniments",
        "add-ins",
        "additions",
        "dippers",
        "extras",
        "fillings",
        "fixings",
        "garnish",
        "garnishes",
        "mix-ins",
        "options",
        "sides",
        "things",
        "toppings",
        "veggies",
    ];
    let mut head = item.to_lowercase().trim_end_matches([',', ' ']).to_string();
    // "toppings of your choice", "mix-in of choice", "toppings you like".
    for qualifier in [" of your choice", " of choice", " you like", " as desired"] {
        if let Some(stripped) = head.strip_suffix(qualifier) {
            head = stripped.to_string();
        }
    }
    let head_word = head
        .trim_end_matches([',', ' '])
        .split_whitespace()
        .last()
        .unwrap_or("")
        .to_string();
    GENERIC_HEADS.contains(&head_word.as_str())
}

/// Purposes after "for" that describe how an ingredient is used, never what
/// it is ("oil for frying", "flour for dusting", "butter for the pan").
const USE_PURPOSES: &[&str] = &[
    "brushing",
    "coating",
    "decorating",
    "deep frying",
    "deep-frying",
    "dipping",
    "drizzling",
    "dusting",
    "finishing",
    "frying",
    "garnish",
    "garnishing",
    "greasing",
    "rolling",
    "serving",
    "shallow frying",
    "sprinkling",
    "the baking sheet",
    "the grill",
    "the knife",
    "the pan",
    "the pans",
    "the pot",
    "the skillet",
    "the work surface",
    "topping",
];

/// Split a trailing usage note that isn't set off by a comma: "for frying",
/// "to taste", "as needed", "if desired", "such as ...", or a bracketed
/// "[see Note]". Returns (item, note) only when a real item remains.
pub(in crate::ingredient_parser) fn split_trailing_phrase_note(
    s: &str,
) -> Option<(String, String)> {
    let lower = s.to_lowercase();
    // Offsets found in the lowercase copy are used on the original, which only
    // lines up when lowercasing kept every byte length.
    if lower.len() != s.len() {
        return None;
    }
    let mut cut: Option<usize> = None;
    let mut consider = |idx: usize| {
        cut = Some(cut.map_or(idx, |c: usize| c.min(idx)));
    };
    // "[see Note]" or "[optional]", but not a synonym that names the food
    // ("crème de framboise [raspberry liqueur]").
    if let Some(idx) = lower.find(" [") {
        let inside = lower.get(idx + 2..).unwrap_or("");
        let inside = inside.split(']').next().unwrap_or("");
        const NOTE_WORDS: &[&str] = &[
            "about",
            "corrected",
            "edited",
            "finish",
            "for",
            "garnish",
            "if",
            "make",
            "more",
            "note",
            "notes",
            "optional",
            "or",
            "see",
            "serve",
            "serving",
            "to",
            "updated",
        ];
        let words: Vec<&str> = inside.split_whitespace().collect();
        let synonym = !words.is_empty()
            && words.len() <= 3
            && words
                .iter()
                .all(|word| word.chars().all(char::is_alphabetic) && !NOTE_WORDS.contains(word));
        if !synonym {
            consider(idx);
        }
    }
    // "neutral oil such as canola" names the food before "such as"; in
    // "Garnishes, such as minced chives, pickles" the examples are the foods.
    if let Some(idx) = lower.find(" such as ") {
        if !has_generic_head(lower.get(..idx).unwrap_or("")) {
            consider(idx);
        }
    }
    for purpose in USE_PURPOSES {
        let phrase = format!(" for {purpose}");
        if let Some(idx) = lower.find(&phrase) {
            let end = idx + phrase.len();
            let rest = lower.get(end..).unwrap_or("");
            // Whole words only: "for serving" but not "for servings of". And
            // a list that goes on ("sugar, for dusting, and mint leaves")
            // names more foods, so the phrase isn't the end of the line.
            let after = rest.trim_start_matches([',', ' ']);
            let next = after
                .strip_prefix("and ")
                .or_else(|| after.strip_prefix("or "));
            // "for deep-frying, and coating the bowl" goes on with another use.
            let continues_list = next.is_some_and(|next| {
                !USE_PURPOSES
                    .iter()
                    .any(|purpose| next.starts_with(purpose.trim_start_matches("the ")))
            });
            // "toppings for serving: extra cheese, ..." introduces a list.
            let introduces_list = rest.trim_start().starts_with(':');
            if rest.chars().next().is_none_or(|c| !c.is_alphanumeric())
                && !continues_list
                && !introduces_list
            {
                consider(idx);
            }
        }
    }
    for suffix in [" to taste", " as needed", " if desired", " optional"] {
        if let Some(idx) = lower.rfind(suffix) {
            if lower
                .get(idx + suffix.len()..)
                .is_some_and(|rest| rest.trim_matches(|c: char| !c.is_alphanumeric()).is_empty())
            {
                consider(idx);
            }
        }
    }
    let mut idx = cut?;
    // "sugar or to taste", "salt or more to taste": the connector belongs to
    // the note.
    for connector in [
        " more or less",
        " or more",
        " or less",
        " plus more",
        " or",
        " and",
    ] {
        let before = lower.get(..idx)?.trim_end();
        if before.ends_with(connector) {
            idx = before.len() - connector.len();
        }
    }
    let item = s.get(..idx)?.trim().trim_end_matches(',').trim();
    let note = s
        .get(idx..)?
        .trim()
        .trim_matches(|c| c == '[' || c == ']')
        .trim();
    if item.is_empty() || note.is_empty() || is_only_prep_words(item) {
        return None;
    }
    Some((item.to_string(), note.replace(['[', ']'], "")))
}
