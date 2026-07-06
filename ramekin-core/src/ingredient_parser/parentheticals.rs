//! Parenthetical segment handling: measurements, notes, and item identity.

use super::*;

/// Parse parenthetical content that may contain multiple measurements.
/// Handles formats like "8 ounces; 227 g each" or "113g, 1/2 cup" or "8 ounces or 225 grams"
#[cfg(test)]
pub(super) fn parse_parenthetical_measurements(content: &str) -> Vec<Measurement> {
    parse_parenthetical_measurement_details(content).measurements
}

pub(super) fn parse_parenthetical_measurement_details(
    content: &str,
) -> ParentheticalMeasurementParse {
    let mut results = Vec::new();
    let mut has_non_measurement_content = parenthetical_measurement_has_note_qualifier(content);

    // Check if the content ends with "each" - this applies to ALL measurements
    // e.g., "8 ounces; 227 g each" means both are per-item
    let content_lower = content.to_lowercase();
    let has_trailing_each = content_lower.trim().ends_with(" each")
        || content_lower.trim().ends_with(";each")
        || content_lower.trim().ends_with(",each");

    // First, normalize " or " to ";" for splitting (but not "or" within words)
    let normalized = normalize_parenthetical_measurement_separators(content)
        .replace(" or ", ";")
        .replace(" Or ", ";")
        .replace(" OR ", ";");

    // Split by semicolons or commas (common separators in recipe measurements)
    for part in normalized.split([';', ',']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some(mut m) = try_parse_parenthetical_measurement(part) {
            // If the entire parenthetical had trailing "each", apply it to all measurements
            // unless this measurement already has "each"
            if has_trailing_each {
                if let Some(ref unit) = m.unit {
                    if !unit.ends_with(" each") {
                        m.unit = Some(format!("{} each", unit));
                    }
                }
            }
            results.push(m);
        } else {
            has_non_measurement_content = true;
        }
    }

    ParentheticalMeasurementParse {
        measurements: results,
        has_non_measurement_content,
    }
}

pub(super) fn parenthetical_measurement_has_note_qualifier(content: &str) -> bool {
    let normalized = content.trim_start().to_lowercase();
    normalized.starts_with('@')
        || normalized.starts_with('~')
        || normalized.starts_with("about ")
        || normalized.starts_with("approx ")
        || normalized.starts_with("approximately ")
        || normalized.starts_with("roughly ")
}

pub(super) fn normalize_parenthetical_measurement_separators(content: &str) -> String {
    let chars: Vec<char> = content.chars().collect();
    let mut normalized = String::with_capacity(content.len() + 4);
    let mut i = 0;

    while i < chars.len() {
        if chars[i] != '/' {
            normalized.push(chars[i]);
            i += 1;
            continue;
        }

        let prev = chars[..i]
            .iter()
            .rev()
            .find(|c| !c.is_whitespace())
            .copied();
        let next = chars[i + 1..].iter().find(|c| !c.is_whitespace()).copied();

        let is_measurement_separator = prev.is_some_and(|c| c.is_ascii_alphabetic() || c == '.')
            && next.is_some_and(|c| c.is_ascii_digit());

        if is_measurement_separator {
            while normalized.ends_with(' ') {
                normalized.pop();
            }
            normalized.push(';');
            normalized.push(' ');
            i += 1;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            continue;
        }

        normalized.push('/');
        i += 1;
    }

    normalized
}

pub(super) fn try_parse_parenthetical_measurement(s: &str) -> Option<Measurement> {
    let cleaned = strip_measurement_qualifiers(s);
    let (mut amount, after_amount) = extract_amount(&cleaned);
    let (mut unit, mut remaining) = extract_unit(&after_amount);

    if unit.is_none() {
        if let Some(amount_str) = amount.as_deref() {
            if let Some(((replacement_amount, recovered_unit), after_recovered)) =
                try_extract_hyphenated_unit_tail(amount_str, &after_amount)
            {
                amount = Some(replacement_amount);
                unit = Some(recovered_unit);
                remaining = after_recovered;
            }
        }
    }

    let (amount, unit) = match (amount, unit) {
        (Some(amount), Some(unit)) => (amount, unit),
        _ => return None,
    };

    let remaining = remaining.trim();
    let has_each_suffix = remaining.eq_ignore_ascii_case("each");
    if !remaining.is_empty() && !has_each_suffix {
        return None;
    }
    let unit = if has_each_suffix && !unit.ends_with(" each") {
        format!("{} each", unit)
    } else {
        unit
    };

    let normalized_unit = normalize_unit(&unit);
    let normalized_base = normalized_unit
        .strip_suffix(" each")
        .unwrap_or(&normalized_unit);
    if normalized_base == "inch" {
        return None;
    }

    if looks_like_bare_parenthetical_size_unit(&cleaned, normalized_base) {
        return None;
    }

    Some(Measurement {
        amount: Some(amount),
        unit: Some(unit),
    })
}

pub(super) fn looks_like_bare_parenthetical_size_unit(s: &str, normalized_unit: &str) -> bool {
    const BARE_PARENTHEICAL_SIZE_UNITS: &[&str] = &["oz", "lb", "inch", "fl oz"];

    if !BARE_PARENTHEICAL_SIZE_UNITS.contains(&normalized_unit) {
        return false;
    }

    let trimmed = s.trim().trim_end_matches('.').to_lowercase();
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.len() != 1 {
        return false;
    }

    let Some((amount_part, unit_part)) = words[0].split_once('-') else {
        return false;
    };
    if !is_amount_like(amount_part) {
        return false;
    }

    normalize_unit(unit_part.trim_end_matches('.')) == normalized_unit
}

pub(super) fn parenthetical_branch_context(raw_before_parenthetical: &str) -> (bool, bool) {
    let follows_comma = raw_before_parenthetical
        .rfind(',')
        .and_then(|comma_idx| raw_before_parenthetical.get(comma_idx + 1..))
        .is_some_and(segment_starts_with_measurement);

    let before_lower = raw_before_parenthetical.to_lowercase();
    let follows_or = before_lower
        .rfind(" or ")
        .and_then(|or_idx| raw_before_parenthetical.get(or_idx + 4..))
        .is_some_and(segment_contains_measurement);

    (follows_comma, follows_or)
}

pub(super) fn segment_starts_with_measurement(segment: &str) -> bool {
    let (_, after_pre_amount_modifier) = strip_measurement_modifier(segment);
    let (amount, after_amount) = extract_amount(&after_pre_amount_modifier);
    let (_, after_pre_unit_modifier) = strip_measurement_modifier(&after_amount);
    let (unit, _) = extract_unit(&after_pre_unit_modifier);
    amount.is_some() && unit.is_some()
}

pub(super) fn segment_contains_measurement(segment: &str) -> bool {
    let before_comma = segment.split(',').next().unwrap_or(segment);
    let words = before_comma.split_whitespace().collect::<Vec<_>>();

    for start in 0..words.len() {
        if segment_starts_with_measurement(&words[start..].join(" ")) {
            return true;
        }
    }

    false
}

pub(super) fn push_deferred_parenthetical_note(
    notes: &mut Vec<DeferredParentheticalNote>,
    segment: &str,
    follows_comma: bool,
    follows_or: bool,
) {
    let segment = segment.trim();
    if segment.is_empty() || is_parenthetical_price_metadata(segment) {
        return;
    }

    notes.push(DeferredParentheticalNote {
        segment: segment.to_string(),
        follows_comma,
        follows_or,
    });
}

pub(super) fn is_parenthetical_price_metadata(segment: &str) -> bool {
    let Some(price) = segment.trim().strip_prefix('$') else {
        return false;
    };
    let Some((dollars, cents)) = price.split_once('.') else {
        return is_valid_price_dollars(price);
    };

    (dollars.is_empty() || is_valid_price_dollars(dollars))
        && cents.len() == 2
        && cents.chars().all(|c| c.is_ascii_digit())
}

pub(super) fn is_valid_price_dollars(dollars: &str) -> bool {
    let mut saw_digit = false;
    let mut last_was_comma = false;

    for c in dollars.chars() {
        if c.is_ascii_digit() {
            saw_digit = true;
            last_was_comma = false;
        } else if c == ',' && saw_digit && !last_was_comma {
            last_was_comma = true;
        } else {
            return false;
        }
    }

    saw_digit && !last_was_comma
}

pub(super) fn take_last_comma_parenthetical_note(
    notes: &mut Vec<DeferredParentheticalNote>,
) -> Option<String> {
    let index = notes.iter().rposition(|note| note.follows_comma)?;
    Some(notes.remove(index).segment)
}

pub(super) fn take_last_or_parenthetical_note(
    notes: &mut Vec<DeferredParentheticalNote>,
) -> Option<String> {
    let index = notes.iter().rposition(|note| note.follows_or)?;
    Some(notes.remove(index).segment)
}

pub(super) fn prepend_deferred_parenthetical_notes(
    note: &mut Option<String>,
    deferred_notes: &[DeferredParentheticalNote],
) {
    let parenthetical_note = deferred_notes
        .iter()
        .map(|note| note.segment.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    if parenthetical_note.is_empty() {
        return;
    }

    *note = Some(match note.take() {
        Some(existing_note) if existing_note.starts_with("or ") => {
            format!("{}; {}", parenthetical_note, existing_note)
        }
        Some(existing_note) => format!("{}, {}", parenthetical_note, existing_note),
        None => parenthetical_note,
    });
}

pub(super) fn join_segments(left: &str, right: &str) -> String {
    if left.is_empty() {
        right.to_string()
    } else if right.is_empty() {
        left.to_string()
    } else {
        format!("{} {}", left, right)
    }
}

pub(super) fn split_parenthetical_item_identity(content: &str) -> Option<(String, Option<String>)> {
    let cleaned = content.trim().trim_start_matches(',').trim();
    if cleaned.is_empty() {
        return None;
    }

    if let Some(measured_identity) = split_measured_parenthetical_item_identity(cleaned) {
        return Some(measured_identity);
    }

    let split_idx = cleaned
        .char_indices()
        .find_map(|(idx, ch)| (ch == ',' || ch == ';').then_some(idx));
    let (item_segment, note_segment) = match split_idx {
        Some(idx) => {
            let item_segment = cleaned.get(..idx).unwrap_or("").trim();
            let note_segment = cleaned.get(idx + 1..).unwrap_or("").trim();
            (item_segment, Some(note_segment))
        }
        None => (cleaned, None),
    };

    let identity_check_segment = item_segment
        .strip_prefix("or ")
        .or_else(|| item_segment.strip_prefix("Or "))
        .or_else(|| item_segment.strip_prefix("OR "))
        .map(str::trim)
        .unwrap_or(item_segment);

    if !looks_like_parenthetical_item_identity(identity_check_segment) {
        return None;
    }

    Some((
        item_segment.to_string(),
        note_segment
            .filter(|segment| !segment.is_empty())
            .map(str::to_string),
    ))
}

pub(super) fn split_measured_parenthetical_item_identity(
    content: &str,
) -> Option<(String, Option<String>)> {
    let (_, after_pre_amount_modifier) = strip_measurement_modifier(content);
    let (amount, after_amount) = extract_amount(&after_pre_amount_modifier);
    let (_, after_pre_unit_modifier) = strip_measurement_modifier(&after_amount);
    let (unit, after_unit) = extract_unit(&after_pre_unit_modifier);
    if amount.is_none() || unit.is_none() {
        return None;
    }

    let item_and_note = after_unit.trim().trim_start_matches(',').trim();
    if item_and_note.is_empty() {
        return None;
    }

    let item_segment = item_and_note.split([',', ';']).next().unwrap_or("").trim();
    if !looks_like_parenthetical_item_identity(item_segment) {
        return None;
    }

    Some((item_segment.to_string(), Some(content.to_string())))
}

pub(super) fn looks_like_parenthetical_item_identity(s: &str) -> bool {
    const GUIDANCE_PREFIXES: &[&str] = &[
        "about",
        "approx",
        "approximately",
        "around",
        "as",
        "at",
        "depending",
        "for",
        "from",
        "if",
        "less",
        "more",
        "note",
        "or",
        "per",
        "plus",
        "see",
        "to",
        "with",
    ];
    const NON_NOUN_WORDS: &[&str] = &[
        "a", "an", "and", "big", "black", "brown", "chopped", "diced", "green", "large", "mashed",
        "medium", "mini", "orange", "purple", "red", "ripe", "small", "sweet", "the", "tiny",
        "well", "white", "yellow",
    ];

    if s.contains(['(', ')']) {
        return false;
    }

    let mut words = s.split_whitespace().peekable();
    let Some(first_word) = words.peek() else {
        return false;
    };
    let first_normalized = normalize_identity_word(first_word);
    if first_normalized.is_empty() || GUIDANCE_PREFIXES.contains(&first_normalized.as_str()) {
        return false;
    }

    let mut has_noun = false;
    for word in s.split_whitespace() {
        let normalized = normalize_identity_word(word);
        if normalized.is_empty() {
            continue;
        }
        if normalized.chars().any(|c| c.is_ascii_digit()) {
            return false;
        }
        if !normalized
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c == '-')
        {
            return false;
        }
        if NON_NOUN_WORDS.contains(&normalized.as_str()) {
            continue;
        }
        if PREP_NOTES
            .iter()
            .any(|note| normalized == *note || note.starts_with(&format!("{} ", normalized)))
        {
            continue;
        }

        has_noun = true;
    }

    has_noun
}

pub(super) fn normalize_identity_word(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_ascii_alphabetic() && c != '-')
        .to_lowercase()
}

pub(super) fn outside_lacks_item_identity(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return true;
    }

    let (_, after_pre_amount_modifier) = strip_measurement_modifier(s);
    let (_, after_amount) = extract_amount(&after_pre_amount_modifier);
    let (_, after_pre_unit_modifier) = strip_measurement_modifier(&after_amount);
    let (_, after_unit) = extract_unit(&after_pre_unit_modifier);
    let candidate = consume_leading_measurement_continuations(after_unit.trim_start_matches(','));

    candidate.is_empty() || is_only_descriptor_or_prep_words(&candidate)
}

fn consume_leading_measurement_continuations(s: &str) -> String {
    let mut candidate = s.trim().to_string();
    loop {
        let trimmed = candidate.trim_start();
        let after_plus = if let Some(after_plus) = trimmed.strip_prefix('+') {
            after_plus
        } else if trimmed.to_lowercase().starts_with("plus ") {
            trimmed.get(5..).unwrap_or("")
        } else {
            break;
        };
        let Some((_, _, after_measurement, _)) =
            parse_plus_continuation_measurement(after_plus.trim_start())
        else {
            break;
        };
        candidate = after_measurement
            .trim()
            .trim_start_matches(',')
            .trim()
            .to_string();
    }
    candidate
}

pub(super) fn is_only_descriptor_or_prep_words(s: &str) -> bool {
    const DESCRIPTOR_WORDS: &[&str] = &[
        "big", "black", "brown", "green", "jumbo", "large", "medium", "meaty", "mini", "orange",
        "purple", "red", "ripe", "small", "sweet", "tiny", "white", "yellow",
    ];

    let mut saw_word = false;
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if is_only_prep_words(part) {
            saw_word = true;
            continue;
        }

        let words: Vec<&str> = part.split_whitespace().collect();
        if words.is_empty() {
            continue;
        }

        let all_descriptors = words.iter().all(|word| {
            let normalized = word
                .trim_matches(|c: char| !c.is_ascii_alphabetic())
                .to_lowercase();
            DESCRIPTOR_WORDS.contains(&normalized.as_str())
        });
        if !all_descriptors {
            return false;
        }
        saw_word = true;
    }

    saw_word
}

pub(super) fn unwrap_redundant_parentheses(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut cursor = 0;

    while let Some(relative_start) = s.get(cursor..).and_then(|tail| tail.find("((")) {
        let start = cursor + relative_start;
        let Some(close_idx) = find_matching_closing_paren(s, start) else {
            break;
        };

        result.push_str(s.get(cursor..start).unwrap_or(""));
        result.push_str(s.get(start + 1..close_idx).unwrap_or(""));
        cursor = close_idx + 1;
    }

    result.push_str(s.get(cursor..).unwrap_or(""));
    result
}

pub(super) fn find_matching_closing_paren(s: &str, open_idx: usize) -> Option<usize> {
    if !s.get(open_idx..)?.starts_with('(') {
        return None;
    }

    let mut depth = 0;
    for (offset, ch) in s.get(open_idx..)?.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open_idx + offset);
                }
            }
            _ => {}
        }
    }

    None
}

/// Strip common qualifiers from measurement strings, but preserve "each" as a unit suffix.
/// e.g., "about 1 cup" -> "1 cup", "227 g each" -> "227 g each" (preserved)
pub(super) fn strip_measurement_qualifiers(s: &str) -> String {
    // Qualifiers to remove completely (they don't change the meaning)
    let remove_qualifiers = [
        " total",
        " about",
        " approximately",
        " approx",
        " roughly",
        " or so",
    ];

    let mut result = s.to_string();
    for q in remove_qualifiers {
        if let Some(idx) = result.to_lowercase().find(q) {
            result = result.get(..idx).unwrap_or("").to_string();
        }
    }

    // Also handle qualifiers at the start
    let start_qualifiers = ["about ", "approximately ", "approx ", "roughly ", "~", "@"];
    let lower = result.to_lowercase();
    for q in start_qualifiers {
        if lower.starts_with(q) {
            result = result.get(q.len()..).unwrap_or("").to_string();
            break;
        }
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mixed_parenthetical_measurements_preserve_valid_segments() {
        let result = parse_parenthetical_measurements("8-ounce; 225g");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].amount, Some("225".to_string()));
        assert_eq!(result[0].unit, Some("g".to_string()));

        let result = parse_parenthetical_measurements("@ 1.25 lbs or 570g");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].amount, Some("1.25".to_string()));
        assert_eq!(result[0].unit, Some("lbs".to_string()));
        assert_eq!(result[1].amount, Some("570".to_string()));
        assert_eq!(result[1].unit, Some("g".to_string()));
    }

    #[test]
    fn test_mixed_parenthetical_measurements_keep_note_content() {
        let result = parse_ingredient("1 (15-ounce/425g) can tomato sauce");
        assert_eq!(result.item, "tomato sauce");
        assert_eq!(result.note, Some("15-ounce/425g".to_string()));
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("can".to_string()));
        assert_eq!(result.measurements[1].amount, Some("425".to_string()));
        assert_eq!(result.measurements[1].unit, Some("g".to_string()));

        let result = parse_ingredient("1 medium (@ 1.25 lbs or 570g) butternut squash");
        assert_eq!(result.item, "butternut squash");
        assert_eq!(result.note, Some("@ 1.25 lbs or 570g".to_string()));
        assert_eq!(result.measurements.len(), 3);
        assert_eq!(result.measurements[1].amount, Some("1.25".to_string()));
        assert_eq!(result.measurements[1].unit, Some("lb".to_string()));
        assert_eq!(result.measurements[2].amount, Some("570".to_string()));
        assert_eq!(result.measurements[2].unit, Some("g".to_string()));
    }

    #[test]
    fn test_parenthetical_branch_context_uses_nearest_branch_text() {
        assert_eq!(
            parenthetical_branch_context("4 cups vegetable broth, 1 1/2 cups vegetable broth "),
            (true, false)
        );
        assert_eq!(
            parenthetical_branch_context("1/2 cup chopped pecans or 1/2 cup walnuts "),
            (false, true)
        );
        assert_eq!(
            parenthetical_branch_context("1 cup chicken or vegetable stock "),
            (false, false)
        );
        assert_eq!(
            parenthetical_branch_context("1 cup onions, finely chopped "),
            (false, false)
        );
    }

    #[test]
    fn test_parenthetical_note_does_not_expose_comma_noun_phrase_to_prep_strip() {
        let result = parse_ingredient(
            "3/4 cup (40 grams) plain, unseasoned dried breadcrumbs (I used, and recommend, panko, see note above)",
        );
        assert_eq!(result.item, "plain, unseasoned dried breadcrumbs");
        assert_eq!(
            result.note,
            Some("I used, and recommend, panko, see note above".to_string())
        );
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("3/4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
        assert_eq!(result.measurements[1].amount, Some("40".to_string()));
        assert_eq!(result.measurements[1].unit, Some("g".to_string()));
    }

    #[test]
    fn test_conditional_alternate_keeps_parenthetical_condition_with_amount() {
        let result = parse_ingredient(
            "4 cups vegetable broth (for dried but soaked chickpeas), 1 1/2 cups vegetable broth (for cooked chickpeas)",
        );
        assert_eq!(result.item, "vegetable broth");
        assert_eq!(
            result.note,
            Some("for dried but soaked chickpeas; or 1 1/2 cup for cooked chickpeas".to_string())
        );
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_or_alternative_keeps_parenthetical_note_with_branch() {
        let result =
            parse_ingredient("1/2-1 cup chopped pecans (optional) or 1/2-1 cup walnuts (optional)");
        assert_eq!(result.item, "chopped pecans");
        assert_eq!(
            result.note,
            Some("optional; or 1/2-1 cup walnuts (optional)".to_string())
        );
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1/2-1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_hyphenated_parenthetical_measurement_preserves_unit() {
        let result = parse_ingredient("1-pound (454-gram) package phyllo/filo pastry");
        assert_eq!(result.item, "phyllo/filo pastry");
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(
            result.measurements[0].unit,
            Some("1-pound package".to_string())
        );
        assert_eq!(result.measurements[1].amount, Some("454".to_string()));
        assert_eq!(result.measurements[1].unit, Some("g".to_string()));
    }

    #[test]
    fn test_parenthetical_yield_hint_does_not_create_alt_measurement() {
        let result = parse_ingredient("1/4 cup freshly squeezed lemon juice (2 lemons)");
        assert_eq!(result.item, "freshly squeezed lemon juice");
        assert_eq!(result.note, Some("2 lemons".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1/4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_parenthetical_slash_separated_weight_alternates_are_preserved() {
        let result = parse_ingredient("1 cup (5 ounces/142 grams) all-purpose flour");
        assert_eq!(result.item, "all-purpose flour");
        assert_eq!(result.measurements.len(), 3);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
        assert_eq!(result.measurements[1].amount, Some("5".to_string()));
        assert_eq!(result.measurements[1].unit, Some("oz".to_string()));
        assert_eq!(result.measurements[2].amount, Some("142".to_string()));
        assert_eq!(result.measurements[2].unit, Some("g".to_string()));
    }

    #[test]
    fn test_parenthetical_compact_metric_range_is_preserved() {
        let result = parse_ingredient("1 to 2 tablespoons (15 to 30g) unsalted butter");
        assert_eq!(result.item, "unsalted butter");
        assert_eq!(result.measurements.len(), 2);
        assert_eq!(result.measurements[0].amount, Some("1 to 2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("tbsp".to_string()));
        assert_eq!(result.measurements[1].amount, Some("15 to 30".to_string()));
        assert_eq!(result.measurements[1].unit, Some("g".to_string()));
    }

    #[test]
    fn test_parenthetical_size_hint_becomes_note() {
        let result = parse_ingredient("12 (6-inch) flour tortillas, warmed");
        assert_eq!(result.item, "flour tortillas, warmed");
        assert_eq!(result.note, Some("6-inch".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("12".to_string()));
        assert_eq!(result.measurements[0].unit, None);
    }

    #[test]
    fn test_parenthetical_package_size_is_not_treated_as_recipe_amount() {
        let result = parse_ingredient(
            "1/2 cup cooked black beans from one (15-ounce) can, drained and rinsed",
        );
        assert_eq!(result.item, "cooked black beans from one can");
        assert_eq!(
            result.note,
            Some("15-ounce, drained and rinsed".to_string())
        );
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1/2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_parenthetical_temperature_hint_becomes_note() {
        let result = parse_ingredient("1 3/4 cups warm water (about 100 degrees)");
        assert_eq!(result.item, "warm water");
        assert_eq!(result.note, Some("about 100 degrees".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1 3/4".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_parenthetical_temperature_hint_preserves_trailing_note() {
        let result = parse_ingredient(
            "2.5 cups warm water (85 degrees F), divided, plus more as needed for feeding",
        );
        assert_eq!(result.item, "warm water, divided");
        assert_eq!(
            result.note,
            Some("85 degrees F, plus more as needed for feeding".to_string())
        );
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("2.5".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_nested_parenthetical_note_is_preserved() {
        let result = parse_ingredient("2 tbsp milk (, full fat (low fat ok too))");
        assert_eq!(result.item, "milk");
        assert_eq!(result.note, Some("full fat (low fat ok too)".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("2".to_string()));
        assert_eq!(result.measurements[0].unit, Some("tbsp".to_string()));
    }

    #[test]
    fn test_parenthetical_item_identity_is_preserved_as_item_text() {
        let result = parse_ingredient("3 large (ripe bananas, well mashed (about 1 1/2 cups))");
        assert_eq!(result.item, "ripe bananas");
        assert_eq!(
            result.note,
            Some("well mashed (about 1 1/2 cups)".to_string())
        );
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("3".to_string()));
        assert_eq!(result.measurements[0].unit, Some("large".to_string()));
    }

    #[test]
    fn test_parenthetical_item_identity_allows_punctuation() {
        let result = parse_ingredient("3 cloves ((the spice cloves!))");
        assert_eq!(result.item, "the spice cloves!");
        assert_eq!(result.note, None);
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("3".to_string()));
        assert_eq!(result.measurements[0].unit, Some("clove".to_string()));
    }

    #[test]
    fn test_parenthetical_item_identity_promotes_after_color_descriptor() {
        let result = parse_ingredient("2 red (yellow or orange bell peppers, stemmed and seeded)");
        assert_eq!(result.item, "red yellow or orange bell peppers");
        assert_eq!(result.note, Some("stemmed and seeded".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("2".to_string()));
        assert_eq!(result.measurements[0].unit, None);
    }

    #[test]
    fn test_parenthetical_item_identity_promotes_after_color_with_unit() {
        let result = parse_ingredient("3 slices white (or wheat sandwich bread, crusts removed)");
        assert_eq!(result.item, "white or wheat sandwich bread");
        assert_eq!(result.note, Some("crusts removed".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("3".to_string()));
        assert_eq!(result.measurements[0].unit, Some("slice".to_string()));
    }

    #[test]
    fn test_parenthetical_approximate_quantity_guidance_becomes_note() {
        let result = parse_ingredient("5 cups thinly sliced cabbage (about half a small head)");
        assert_eq!(result.item, "thinly sliced cabbage");
        assert_eq!(result.note, Some("about half a small head".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("5".to_string()));
        assert_eq!(result.measurements[0].unit, Some("cup".to_string()));
    }

    #[test]
    fn test_parenthetical_approximate_yield_hint_becomes_note() {
        let result = parse_ingredient("1 rack beef back ribs (about 7 bones)");
        assert_eq!(result.item, "rack beef back ribs");
        assert_eq!(result.note, Some("about 7 bones".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, None);
    }

    #[test]
    fn test_parenthetical_price_metadata_is_not_preserved_as_note() {
        let result = parse_ingredient("6 ounces vanilla wafers ($1.43)");
        assert_eq!(result.item, "vanilla wafers");
        assert_eq!(result.note, None);
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("6".to_string()));
        assert_eq!(result.measurements[0].unit, Some("oz".to_string()));
    }

    #[test]
    fn test_parenthetical_cents_only_price_metadata_is_not_preserved_as_note() {
        let result = parse_ingredient("3 bananas ($.99)");
        assert_eq!(result.item, "bananas");
        assert_eq!(result.note, None);
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("3".to_string()));
        assert_eq!(result.measurements[0].unit, None);
    }

    #[test]
    fn test_parenthetical_price_metadata_does_not_pollute_other_notes() {
        let result = parse_ingredient("5.5 lb. bone-in turkey breast (skin on)** ($19.25)");
        assert_eq!(result.item, "bone-in turkey breast");
        assert_eq!(result.note, Some("skin on".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("5.5".to_string()));
        assert_eq!(result.measurements[0].unit, Some("lb".to_string()));
    }

    #[test]
    fn test_parenthetical_price_metadata_preserves_trailing_prep_note() {
        let result = parse_ingredient("8 Tbsp butter, room temperature ($1.12)");
        assert_eq!(result.item, "butter");
        assert_eq!(result.note, Some("room temperature".to_string()));
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("8".to_string()));
        assert_eq!(result.measurements[0].unit, Some("tbsp".to_string()));
    }
}
