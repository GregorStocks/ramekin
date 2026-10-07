//! Citrus lines naming the zest or juice ("Juice of half a lemon", "Finely
//! grated zest and juice of 1 lime", "1 lemon, juiced", "2 limes (zested and
//! juiced)"): the fruit is the counted unit and the item names the part, so
//! "Juice of 1 lemon" and "1 lemon, juiced" are both 1 lemon of lemon juice.

use std::sync::LazyLock;

use regex::Regex;

use super::{
    extract_amount, normalize_unicode, normalize_word_numbers, parse_ingredient,
    strip_leading_list_marker, Measurement, ParsedIngredient,
};
use crate::text::decode_html_entities;

/// "[the] [prep words] zest|juice|zest and juice of <rest>".
static PART_OF_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:the\s+)?(?P<prep>(?:(?:finely|freshly|fresh|grated|finely-grated|freshly-grated|squeezed)\s+)*)(?P<parts>zest\s*(?:and|&)\s*juice|juice\s*(?:and|&)\s*zest|zest|juice)\s+of\s+(?P<rest>.+)$",
    )
    .expect("Invalid citrus part regex")
});

/// "[size] <fruit>[s]<trailer>", after the amount.
static FRUIT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?P<unit>(?:(?:small|medium|large|extra[- ]large)\s+)?(?P<fruit>meyer\s+lemon|key\s+lime|pink\s+grapefruit|lemon|lime|orange|grapefruit|clementine|tangerine)(?:e?s)?)\b(?P<trailer>.*)$",
    )
    .expect("Invalid citrus fruit regex")
});

/// "[,] [(] [finely|freshly] juiced|zested|zested and juiced|juiced and
/// zested <rest>", the tail after "<count> <citrus>".
static PREP_TAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^\s*,?\s*(?P<open>\(*)\s*(?P<prep>(?:(?:finely|freshly)\s+)?)(?P<parts>zested\s+(?:and|&)\s+juiced|juiced\s+(?:and|&)\s+zested|zested|juiced)(?P<rest>.*)$",
    )
    .expect("Invalid citrus prep tail regex")
});

/// Parse a citrus line naming the zest or juice ("Juice of 1 lemon", "1
/// lemon, juiced", "2 limes (zested and juiced)") into one ingredient per
/// part it names ("zest and juice" is two), or None for any other line.
pub(super) fn parse_citrus_part_line(raw: &str) -> Option<Vec<ParsedIngredient>> {
    let decoded = decode_html_entities(raw.trim());
    let normalized = normalize_unicode(&decoded);
    let line = strip_leading_list_marker(&normalized);
    let line = line.trim();
    let citrus = parse_part_of(line).or_else(|| parse_prep_tail(line))?;
    citrus.ingredients(raw)
}

/// A citrus line taken apart: "<prep> <parts> of <amount> <unit><trailer>".
struct CitrusLine {
    amount: String,
    /// The size and fruit as written: "lemons", "large lemon".
    unit: String,
    fruit: String,
    prep: String,
    parts: String,
    trailer: String,
}

/// "Zest and juice of 1 lemon".
fn parse_part_of(line: &str) -> Option<CitrusLine> {
    let captures = PART_OF_REGEX.captures(line)?;
    let rest = normalize_word_numbers(captures["rest"].trim());
    let (amount, after_amount) = match strip_article(&rest) {
        Some(after) => (Some("1".to_string()), after.to_string()),
        None => extract_amount(&rest),
    };
    let after_amount = strip_article(&after_amount).unwrap_or(&after_amount);
    let fruit_captures = FRUIT_REGEX.captures(after_amount.trim())?;
    Some(CitrusLine {
        amount: amount?,
        unit: collapse_whitespace(&fruit_captures["unit"]).to_lowercase(),
        fruit: collapse_whitespace(&fruit_captures["fruit"]).to_lowercase(),
        prep: collapse_whitespace(&captures["prep"]).to_lowercase(),
        parts: captures["parts"].to_lowercase(),
        trailer: fruit_captures["trailer"].trim_end().to_string(),
    })
}

/// "1 lemon, juiced", "2 limes (zested and juiced, about 1/4 cup)": the
/// juiced/zested tail is rebuilt as an ordinary trailer (" (about 1/4 cup)").
/// The phrase must end the tail or be followed by punctuation, so "1 lime,
/// juiced and zest freshly grated" and "1 lemon (zested for topping)" are
/// left alone.
fn parse_prep_tail(line: &str) -> Option<CitrusLine> {
    let line = normalize_word_numbers(line);
    let (amount, after_amount) = extract_amount(&line);
    let amount = amount?;
    let fruit_captures = FRUIT_REGEX.captures(after_amount.trim())?;
    // Only fruits whose juice and zest the catalog can count per fruit
    // (bespoke.json); "1 grapefruit (juiced)" stays the whole fruit, which
    // at least resolves, until a grapefruit juice yield exists.
    let fruit = collapse_whitespace(&fruit_captures["fruit"]).to_lowercase();
    if !["lemon", "lime", "orange"].contains(&fruit.as_str()) {
        return None;
    }
    let tail = PREP_TAIL_REGEX.captures(&fruit_captures["trailer"])?;
    let rest = tail["rest"].trim_end();
    if rest.trim_start().starts_with(|c: char| c.is_alphanumeric()) {
        return None;
    }
    let opened = tail["open"].len();
    let trailer = if opened == 0 {
        rest.to_string()
    } else {
        // Close the parens the phrase opened; what was inside them alongside
        // the phrase ("about 1/4 cup") stays parenthesized.
        let (inside, after) = rest.split_at_checked(closing_paren_index(rest, opened)?)?;
        // Leading ")"s close the extra parens of "((juiced))".
        let inside = inside
            .trim_start_matches(|c: char| c == ')' || c == ',' || c.is_whitespace())
            .trim_end();
        let after = after.strip_prefix(')')?;
        if inside.is_empty() {
            after.to_string()
        } else {
            format!(" ({inside}){after}")
        }
    };
    Some(CitrusLine {
        amount,
        unit: collapse_whitespace(&fruit_captures["unit"]).to_lowercase(),
        fruit,
        prep: collapse_whitespace(&tail["prep"]).to_lowercase(),
        parts: tail["parts"].to_lowercase(),
        trailer,
    })
}

/// Byte index of the `)` closing `depth` already-open parens, or None if
/// they never close.
fn closing_paren_index(s: &str, depth: usize) -> Option<usize> {
    let mut depth = depth;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

impl CitrusLine {
    fn ingredients(&self, raw: &str) -> Option<Vec<ParsedIngredient>> {
        let trailer = self.trailer.as_str();
        if !trailer_is_note(trailer) {
            return None;
        }
        let has_zest = self.parts.contains("zest");
        let has_juice = self.parts.contains("juice");

        let mut ingredients = Vec::new();
        for (part, wanted) in [("zest", has_zest), ("juice", has_juice)] {
            if !wanted {
                continue;
            }
            // Prep words describe the zest ("grated zest and juice of 1 lemon").
            let prep = (part == "zest" || !has_zest)
                .then_some(self.prep.as_str())
                .filter(|prep| !prep.is_empty());
            // On a "zest and juice" line, a trailer naming one part ("(zest
            // reserved for serving)") is that part's, and a volume ("(about 2
            // tablespoons)") measures the juice.
            let both = has_zest && has_juice;
            let trailer = if both && names_only_other_part(trailer, part) {
                ""
            } else {
                trailer
            };
            let keep_trailer_measurements = !both || part == "juice";
            ingredients.push(part_ingredient(
                &format!("{} {part}", self.fruit),
                Measurement {
                    amount: Some(self.amount.clone()),
                    unit: Some(self.unit.clone()),
                },
                prep,
                trailer,
                keep_trailer_measurements,
                raw,
            )?);
        }
        Some(ingredients)
    }
}

/// One part's ingredient; a measurement in the trailer leads. The trailer (", or more to taste", " (2 to 3
/// tablespoons)", " or lime (about 2 tablespoons)") is parsed after the item
/// so its measurements and notes come out the usual way; words left after
/// the item ("or lime") join the note. None if the parse cuts into the item.
fn part_ingredient(
    item: &str,
    measurement: Measurement,
    prep: Option<&str>,
    trailer: &str,
    keep_trailer_measurements: bool,
    raw: &str,
) -> Option<ParsedIngredient> {
    let mut measurements = vec![measurement];
    let mut notes: Vec<String> = prep.map(str::to_string).into_iter().collect();
    if !trailer.is_empty() {
        let parsed = parse_ingredient(&format!("{item}{trailer}"));
        let leftover = parsed.item.strip_prefix(item)?;
        // An explicit volume ("(about 3 tablespoons)") is the author's
        // measure, so it comes first and the fruit count is the alternative.
        if keep_trailer_measurements {
            measurements.splice(0..0, parsed.measurements);
        }
        notes.extend(
            [
                leftover.trim_start_matches(|c: char| c == ',' || c.is_whitespace()),
                parsed.note.as_deref().unwrap_or_default(),
            ]
            .into_iter()
            .map(str::trim)
            .filter(|note| !note.is_empty())
            .map(str::to_string),
        );
    }
    let note = notes.join(", ");
    Some(ParsedIngredient {
        item: item.to_string(),
        measurements,
        note: (!note.is_empty()).then_some(note),
        raw: Some(raw.to_string()),
        section: None,
    })
}

/// Whether the trailer names the other part ("zest" or "juice") and not this one.
fn names_only_other_part(trailer: &str, part: &str) -> bool {
    let lower = trailer.to_lowercase();
    let other = if part == "zest" { "juice" } else { "zest" };
    lower.contains(other) && !lower.contains(part)
}

/// Text after "a"/"an" ("half a lemon" is "1/2 a lemon" by now).
fn strip_article(s: &str) -> Option<&str> {
    let s = s.trim_start();
    let (word, rest) = s.split_once(char::is_whitespace)?;
    (word.eq_ignore_ascii_case("a") || word.eq_ignore_ascii_case("an")).then(|| rest.trim_start())
}

/// Whether what follows the fruit is a note rather than more of the name:
/// nothing, punctuation (", see notes", "(to taste)"), or an "or" alternative.
/// "Juice of 2 lime wedges" counts wedges, not limes, so it isn't one of these,
/// and neither is a measured addition (", plus 1 tablespoon lemon juice"),
/// which is another ingredient.
fn trailer_is_note(trailer: &str) -> bool {
    let trimmed = trailer.trim_start();
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    let adds_measured_ingredient = words.windows(2).any(|pair| {
        pair[0].eq_ignore_ascii_case("plus")
            && pair[1].chars().next().is_some_and(|c| c.is_ascii_digit())
    });
    if adds_measured_ingredient {
        return false;
    }
    match trimmed.chars().next() {
        None => true,
        Some(c) if !c.is_alphanumeric() => true,
        Some(_) => trimmed
            .split_whitespace()
            .next()
            .is_some_and(|word| word.eq_ignore_ascii_case("or")),
    }
}

fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_one(line: &str) -> ParsedIngredient {
        let mut parsed = parse_citrus_part_line(line).expect("citrus line");
        assert_eq!(parsed.len(), 1, "{line}");
        parsed.remove(0)
    }

    fn measurement(amount: &str, unit: &str) -> Measurement {
        Measurement {
            amount: Some(amount.to_string()),
            unit: Some(unit.to_string()),
        }
    }

    #[test]
    fn juice_of_counted_fruit() {
        for (line, amount, unit, item) in [
            ("Juice of 1 lemon", "1", "lemon", "lemon juice"),
            ("Juice of half a lemon", "1/2", "lemon", "lemon juice"),
            ("Juice of ½ lemon", "1/2", "lemon", "lemon juice"),
            ("juice of one orange", "1", "orange", "orange juice"),
            ("Juice of 2 limes", "2", "limes", "lime juice"),
            (
                "Juice of 1 medium lemon",
                "1",
                "medium lemon",
                "lemon juice",
            ),
            ("the juice of one lime", "1", "lime", "lime juice"),
            (
                "zest of 2 Large   Lemons",
                "2",
                "large lemons",
                "lemon zest",
            ),
            (
                "Zest of 1 1/2 pink grapefruits",
                "1 1/2",
                "pink grapefruits",
                "pink grapefruit zest",
            ),
            ("zest of two oranges", "2", "oranges", "orange zest"),
            ("Zest of 1/4 lemon", "1/4", "lemon", "lemon zest"),
            ("\u{a0}Zest of 2 lemons", "2", "lemons", "lemon zest"),
        ] {
            let parsed = parse_one(line);
            assert_eq!(parsed.item, item, "{line}");
            assert_eq!(
                parsed.measurements,
                vec![measurement(amount, unit)],
                "{line}"
            );
            assert_eq!(parsed.note, None, "{line}");
            assert_eq!(parsed.raw.as_deref(), Some(line));
        }
    }

    #[test]
    fn prep_words_and_trailers_become_notes() {
        let parsed = parse_one("Finely grated zest of 1 lemon");
        assert_eq!(parsed.item, "lemon zest");
        assert_eq!(parsed.note.as_deref(), Some("finely grated"));

        let parsed = parse_one("Juice of half a lemon, or more to taste");
        assert_eq!(parsed.item, "lemon juice");
        assert_eq!(parsed.measurements, vec![measurement("1/2", "lemon")]);
        assert_eq!(parsed.note.as_deref(), Some("or more to taste"));

        let parsed = parse_one("zest of 1 lime (to taste)");
        assert_eq!(parsed.item, "lime zest");
        assert_eq!(parsed.note.as_deref(), Some("to taste"));
    }

    #[test]
    fn trailing_measurement_is_an_alternative() {
        let parsed = parse_one("Juice of half a lime (2 to 3 tablespoons)");
        assert_eq!(parsed.item, "lime juice");
        assert_eq!(parsed.measurements[1], measurement("1/2", "lime"));
        assert_eq!(parsed.measurements.len(), 2);
    }

    #[test]
    fn measurement_after_an_alternative_fruit_is_kept() {
        let parsed =
            parse_one("Juice of 1 lemon or lime (about 2 tablespoons), plus more to taste");
        assert_eq!(parsed.item, "lemon juice");
        assert_eq!(parsed.measurements[1], measurement("1", "lemon"));
        assert_eq!(parsed.measurements.len(), 2, "{:?}", parsed.measurements);
        assert_eq!(
            parsed.note.as_deref(),
            Some("or lime, about 2 tablespoons, plus more to taste")
        );
    }

    #[test]
    fn zest_and_juice_is_two_ingredients() {
        let parsed = parse_citrus_part_line("Finely grated zest and juice of 1/2 lime").unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].item, "lime zest");
        assert_eq!(parsed[0].note.as_deref(), Some("finely grated"));
        assert_eq!(parsed[1].item, "lime juice");
        assert_eq!(parsed[1].note, None);
        for part in &parsed {
            assert_eq!(part.measurements, vec![measurement("1/2", "lime")]);
        }

        let parsed = parse_citrus_part_line("Zest &amp; juice of 1 Lemon").unwrap();
        assert_eq!(parsed[0].item, "lemon zest");
        assert_eq!(parsed[1].item, "lemon juice");
        assert_eq!(parsed[1].measurements, vec![measurement("1", "lemon")]);
    }

    #[test]
    fn zest_and_juice_trailer_goes_to_its_part() {
        let parsed =
            parse_citrus_part_line("Zest and juice of 2 lemons (zest reserved for serving)")
                .unwrap();
        assert_eq!(parsed[0].note.as_deref(), Some("zest reserved for serving"));
        assert_eq!(parsed[1].note, None);

        let parsed =
            parse_citrus_part_line("Zest and juice of 1 lime (about 2 tablespoons)").unwrap();
        assert_eq!(parsed[0].measurements, vec![measurement("1", "lime")]);
        assert_eq!(parsed[1].measurements.len(), 2);

        let parsed =
            parse_citrus_part_line("Zest and juice of 1 lime (plus more for serving)").unwrap();
        assert_eq!(parsed[0].note, parsed[1].note);
        assert!(parsed[0].note.is_some());
    }

    #[test]
    fn juiced_or_zested_fruit() {
        for (line, amount, unit, item, note) in [
            ("1 lemon, juiced", "1", "lemon", "lemon juice", None),
            ("2 limes, juiced", "2", "limes", "lime juice", None),
            ("1&nbsp;lemon, juiced", "1", "lemon", "lemon juice", None),
            ("½  lemon juiced", "1/2", "lemon", "lemon juice", None),
            ("1  lime (juiced)", "1", "lime", "lime juice", None),
            ("1  lemon, (juiced)", "1", "lemon", "lemon juice", None),
            ("1  limes ((juiced))", "1", "limes", "lime juice", None),
            (
                "2 medium lemons, juiced",
                "2",
                "medium lemons",
                "lemon juice",
                None,
            ),
            ("1  orange (zested)", "1", "orange", "orange zest", None),
            (
                "1  lemon finely zested",
                "1",
                "lemon",
                "lemon zest",
                Some("finely"),
            ),
            (
                "2  limes, (freshly juiced)",
                "2",
                "limes",
                "lime juice",
                Some("freshly"),
            ),
            (
                "½  lemon juiced, to serve",
                "1/2",
                "lemon",
                "lemon juice",
                Some("to serve"),
            ),
            (
                "½ lemon, zested, plus more for garnish",
                "1/2",
                "lemon",
                "lemon zest",
                Some("plus more for garnish"),
            ),
            (
                "1/2 lemon (juiced) (see note)",
                "1/2",
                "lemon",
                "lemon juice",
                Some("see note"),
            ),
        ] {
            let parsed = parse_one(line);
            assert_eq!(parsed.item, item, "{line}");
            assert_eq!(
                parsed.measurements,
                vec![measurement(amount, unit)],
                "{line}"
            );
            assert_eq!(parsed.note.as_deref(), note, "{line}");
            assert_eq!(parsed.raw.as_deref(), Some(line));
        }
    }

    #[test]
    fn juiced_volume_leads() {
        for line in [
            "1 large lemon, juiced (about 1/4 cup)",
            "1  lemon (juiced, about 1/4 cup)",
        ] {
            let parsed = parse_one(line);
            assert_eq!(parsed.item, "lemon juice", "{line}");
            assert_eq!(parsed.measurements[0], measurement("1/4", "cup"), "{line}");
            assert_eq!(parsed.measurements.len(), 2, "{line}");
        }
    }

    #[test]
    fn zested_and_juiced_is_two_ingredients() {
        for line in [
            "1 lemon, zested and juiced",
            "1  lemon (juiced and zested)",
            "2 lemons, juiced and zested",
        ] {
            let parsed = parse_citrus_part_line(line).unwrap();
            assert_eq!(parsed.len(), 2, "{line}");
            assert_eq!(parsed[0].item, "lemon zest", "{line}");
            assert_eq!(parsed[1].item, "lemon juice", "{line}");
        }
    }

    #[test]
    fn other_lines_are_left_alone() {
        for line in [
            "Juice of the above lemon",
            "Juice of 2 lime wedges",
            "the zest of half a lemon and the juice of a whole one",
            "Most of the finely grated zest and all of the juice of 1 lemon",
            "1 lemon, zest and juice",
            "2 tablespoons lemon juice",
            "zest of 1 lemongrass stalk",
            "Grated zest of 1 lemon, plus 1 tablespoon lemon juice",
            "1 lemon",
            "2 lemons, sliced",
            "1  lime, (juiced and zest freshly grated)",
            "1  lemon (zested for topping)",
            "3 limes (1/2 lime zested and 3 limes juiced)",
            "2 lemons, 1 juiced, 1 sliced into half-moons",
            "1  large orange or 2 small ones, juiced",
            "2 oranges juiced, plus 2 slices to garnish",
            "3 cups fresh clementine juice (about 12 clementines, juiced)",
            "1 lemon juice",
            "1 large grapefruit (juiced)",
        ] {
            assert_eq!(parse_citrus_part_line(line), None, "{line}");
        }
    }
}
